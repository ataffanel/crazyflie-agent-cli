use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use crazyflie_lib::subsystems::log::LogPeriod;
use crazyflie_lib::{Crazyflie, Value, ValueType};
use crazyflie_link::LinkContext;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::output::{print_tagged, Tag};
use crate::protocol::{LogVarInfo, ParamInfo, Request, Response};
use crate::toc_cache::FileTocCache;

/// Derive a socket path from a Crazyflie URI.
/// The socket is placed at `/tmp/crazyflie-agent-<id>.sock` where `<id>` is
/// the alphanumeric characters from the URI.
fn socket_path(uri: &str) -> PathBuf {
    let id: String = uri.chars().filter(|c| c.is_alphanumeric()).collect();
    PathBuf::from(format!("/tmp/crazyflie-agent-{}.sock", id))
}

/// Scan `/tmp/` for any active `crazyflie-agent-*.sock` file.
pub fn find_socket_path() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/tmp").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("crazyflie-agent-") && name.ends_with(".sock") {
                return Some(path);
            }
        }
    }
    None
}

/// Main daemon entry point. Connects to a Crazyflie and serves requests over a
/// Unix socket.
pub async fn run(uri: &str) -> Result<()> {
    let sock_path = socket_path(uri);

    // Clean up stale socket file if it exists
    if sock_path.exists() {
        std::fs::remove_file(&sock_path)?;
    }

    // Connect to the Crazyflie
    let link_context = LinkContext::new();
    let cf = Crazyflie::connect_from_uri(&link_context, uri, FileTocCache::new()).await?;
    let firmware_version = cf.platform.firmware_version().await.unwrap_or_else(|_| "unknown".to_string());

    print_tagged(Tag::Status, &format!("connected uri={} firmware={}", uri, firmware_version));

    let cf = Arc::new(cf);

    // Shared handle to the active log task so it can be cancelled
    let active_log: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    // Spawn console streaming task
    {
        let cf_console = cf.clone();
        tokio::spawn(async move {
            let mut stream = cf_console.console.line_stream().await;
            while let Some(line) = stream.next().await {
                print_tagged(Tag::Console, &line);
            }
        });
    }

    // Shutdown signal
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Bind Unix socket listener
    let listener = UnixListener::bind(&sock_path)?;
    print_tagged(Tag::Status, &format!("listening on {}", sock_path.display()));

    // Accept and handle client connections
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let cf_client = cf.clone();
                        let uri_owned = uri.to_string();
                        let fw_version = firmware_version.clone();
                        let shutdown = shutdown_tx.clone();
                        let log_handle = active_log.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(stream, cf_client, uri_owned, fw_version, shutdown, log_handle).await {
                                print_tagged(Tag::Error, &format!("client error: {}", e));
                            }
                        });
                    }
                    Err(e) => {
                        print_tagged(Tag::Error, &format!("accept error: {}", e));
                        break;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                print_tagged(Tag::Status, "shutting down");
                break;
            }
        }
    }

    // Clean up socket file on exit
    let _ = std::fs::remove_file(&sock_path);
    Ok(())
}

/// Handle one client connection: read one JSON request, send one JSON response.
async fn handle_client(
    stream: tokio::net::UnixStream,
    cf: Arc<Crazyflie>,
    uri: String,
    firmware_version: String,
    shutdown: tokio::sync::mpsc::Sender<()>,
    active_log: Arc<Mutex<Option<JoinHandle<()>>>>,
) -> Result<()> {
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    let request: Request = serde_json::from_str(line)?;
    let response = handle_request(request, cf, uri, firmware_version, shutdown, active_log).await;
    let mut json = serde_json::to_string(&response)?;
    json.push('\n');
    write_half.write_all(json.as_bytes()).await?;
    Ok(())
}

/// Dispatch a Request to the appropriate handler and return a Response.
async fn handle_request(
    request: Request,
    cf: Arc<Crazyflie>,
    uri: String,
    firmware_version: String,
    shutdown: tokio::sync::mpsc::Sender<()>,
    active_log: Arc<Mutex<Option<JoinHandle<()>>>>,
) -> Response {
    match request {
        Request::Stop => {
            print_tagged(Tag::Status, "stop requested");
            // Signal the main loop to shut down after response is sent
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                shutdown.send(()).await.ok();
            });
            Response::Ok { message: "stopping".to_string() }
        }

        Request::Status => Response::Status {
            connected: true,
            uri,
            firmware_version: Some(firmware_version),
        },

        Request::ParamList => {
            let names = cf.param.names();
            let mut params = Vec::with_capacity(names.len());
            for name in names {
                let value_type = match cf.param.get_type(&name) {
                    Ok(t) => format_value_type(t),
                    Err(e) => {
                        return Response::Error { message: format!("param type error for {}: {}", name, e) };
                    }
                };
                let access = match cf.param.is_writable(&name) {
                    Ok(true) => "rw".to_string(),
                    Ok(false) => "ro".to_string(),
                    Err(e) => {
                        return Response::Error { message: format!("param access error for {}: {}", name, e) };
                    }
                };
                let value: Value = match cf.param.get(&name).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Response::Error { message: format!("param get error for {}: {}", name, e) };
                    }
                };
                params.push(ParamInfo {
                    name,
                    value_type,
                    access,
                    value: format_value(value),
                });
            }
            Response::ParamList { params }
        }

        Request::ParamGet { name } => {
            let value: Value = match cf.param.get(&name).await {
                Ok(v) => v,
                Err(e) => return Response::Error { message: format!("param get error: {}", e) },
            };
            Response::ParamValue { name, value: format_value(value) }
        }

        Request::ParamSet { name, value } => {
            match cf.param.is_writable(&name) {
                Ok(false) => return Response::Error { message: format!("param set error: {} is read-only", name) },
                Err(e) => return Response::Error { message: format!("param set error: {}", e) },
                Ok(true) => {}
            }
            match param_set(&cf, &name, &value).await {
                Ok(()) => Response::Ok { message: format!("set {} = {}", name, value) },
                Err(e) => Response::Error { message: format!("param set error: {}", e) },
            }
        }

        Request::LogList => {
            let names = cf.log.names();
            let variables = names
                .into_iter()
                .map(|name| {
                    let value_type = cf
                        .log
                        .get_type(&name)
                        .map(format_value_type)
                        .unwrap_or_else(|_| "unknown".to_string());
                    LogVarInfo { name, value_type }
                })
                .collect();
            Response::LogList { variables }
        }

        Request::LogStart { variables, rate_hz } => {
            // Cancel any previously running log task
            stop_active_log(&active_log).await;

            // period_ms = 1000 / rate_hz, clamped to valid range [10, 2550]
            let period_ms = if rate_hz == 0 { 100 } else { (1000 / rate_hz).max(10).min(2550) };
            match start_log(cf, variables, period_ms, &active_log).await {
                Ok(()) => Response::Ok { message: "log started".to_string() },
                Err(e) => Response::Error { message: format!("log start error: {}", e) },
            }
        }

        Request::LogStop => {
            stop_active_log(&active_log).await;
            Response::Ok { message: "log stopped".to_string() }
        }
    }
}

/// Cancel the currently active log task, if any.
async fn stop_active_log(active_log: &Arc<Mutex<Option<JoinHandle<()>>>>) {
    let mut guard = active_log.lock().await;
    if let Some(handle) = guard.take() {
        handle.abort();
        // Wait for the task to finish so the log block is fully dropped
        // and cleaned up on the Crazyflie before we create a new one.
        let _ = handle.await;
    }
}

/// Start a log block, add variables, and spawn a reader task that prints log lines.
async fn start_log(
    cf: Arc<Crazyflie>,
    variables: Vec<String>,
    period_ms: u64,
    active_log: &Arc<Mutex<Option<JoinHandle<()>>>>,
) -> Result<()> {
    let mut block = cf.log.create_block().await?;
    for var in &variables {
        block.add_variable(var).await?;
    }
    let period = LogPeriod::from_millis(period_ms)?;
    let stream = block.start(period).await?;

    let handle = tokio::spawn(async move {
        loop {
            match stream.next().await {
                Ok(data) => {
                    let ts = data.timestamp as f64 / 1000.0;
                    for (var, val) in &data.data {
                        print_tagged(Tag::Log(ts), &format!("{}={}", var, format_value(*val)));
                    }
                }
                Err(_) => break,
            }
        }
    });

    *active_log.lock().await = Some(handle);
    Ok(())
}

/// Set a parameter using type-aware parsing.
async fn param_set(cf: &Crazyflie, name: &str, value: &str) -> Result<()> {
    let param_type = cf.param.get_type(name)?;
    match param_type {
        ValueType::U8 => cf.param.set(name, value.parse::<u8>()?).await?,
        ValueType::U16 => cf.param.set(name, value.parse::<u16>()?).await?,
        ValueType::U32 => cf.param.set(name, value.parse::<u32>()?).await?,
        ValueType::U64 => cf.param.set(name, value.parse::<u64>()?).await?,
        ValueType::I8 => cf.param.set(name, value.parse::<i8>()?).await?,
        ValueType::I16 => cf.param.set(name, value.parse::<i16>()?).await?,
        ValueType::I32 => cf.param.set(name, value.parse::<i32>()?).await?,
        ValueType::I64 => cf.param.set(name, value.parse::<i64>()?).await?,
        ValueType::F16 | ValueType::F32 => cf.param.set(name, value.parse::<f32>()?).await?,
        ValueType::F64 => cf.param.set(name, value.parse::<f64>()?).await?,
    }
    Ok(())
}

/// Format a `Value` as a human-readable string.
fn format_value(v: Value) -> String {
    match v {
        Value::U8(x) => x.to_string(),
        Value::U16(x) => x.to_string(),
        Value::U32(x) => x.to_string(),
        Value::U64(x) => x.to_string(),
        Value::I8(x) => x.to_string(),
        Value::I16(x) => x.to_string(),
        Value::I32(x) => x.to_string(),
        Value::I64(x) => x.to_string(),
        Value::F16(x) => x.to_string(),
        Value::F32(x) => x.to_string(),
        Value::F64(x) => x.to_string(),
    }
}

/// Format a `ValueType` as a lowercase string label.
fn format_value_type(t: ValueType) -> String {
    match t {
        ValueType::U8 => "u8",
        ValueType::U16 => "u16",
        ValueType::U32 => "u32",
        ValueType::U64 => "u64",
        ValueType::I8 => "i8",
        ValueType::I16 => "i16",
        ValueType::I32 => "i32",
        ValueType::I64 => "i64",
        ValueType::F16 => "f16",
        ValueType::F32 => "f32",
        ValueType::F64 => "f64",
    }
    .to_string()
}
