use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::Commands;
use crate::daemon::find_socket_path;
use crate::protocol::{Request, Response};

async fn send_request(request: Request) -> Result<Response> {
    let sock_path = find_socket_path()
        .context("No active session found. Run 'crazyflie-agent-cli start <uri>' first.")?;

    let stream = UnixStream::connect(&sock_path)
        .await
        .context("Failed to connect to daemon. Is a session running?")?;

    let (reader, mut writer) = stream.into_split();

    let json = serde_json::to_string(&request)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let response: Response = serde_json::from_str(line.trim())?;
    Ok(response)
}

fn print_response(response: &Response) {
    match response {
        Response::Ok { message } => println!("{}", message),
        Response::Error { message } => {
            eprintln!("error: {}", message);
            std::process::exit(1);
        }
        Response::Status { connected, uri, firmware_version } => {
            println!("connected: {}", connected);
            println!("uri: {}", uri);
            if let Some(fw) = firmware_version {
                println!("firmware: {}", fw);
            }
        }
        Response::ParamList { params } => {
            for p in params {
                println!("{}\t{}\t{}\t{}", p.name, p.value_type, p.access, p.value);
            }
        }
        Response::ParamValue { name: _, value } => {
            println!("{}", value);
        }
        Response::LogList { variables } => {
            for v in variables {
                println!("{}\t{}", v.name, v.value_type);
            }
        }
    }
}

pub async fn send_command(cmd: Commands) -> Result<()> {
    let request = match cmd {
        Commands::Stop => Request::Stop,
        Commands::Status => Request::Status,
        Commands::Param { command } => {
            use crate::ParamCommands;
            match command {
                ParamCommands::List => Request::ParamList,
                ParamCommands::Get { name } => Request::ParamGet { name },
                ParamCommands::Set { name, value } => Request::ParamSet { name, value },
            }
        }
        Commands::Log { command } => {
            use crate::LogCommands;
            match command {
                LogCommands::List => Request::LogList,
                LogCommands::Start { variables, rate } => Request::LogStart {
                    variables,
                    rate_hz: rate,
                },
                LogCommands::Stop => Request::LogStop,
            }
        }
        // These are handled directly in main.rs, not via client
        Commands::Start { .. } | Commands::Scan | Commands::Flash { .. }
        | Commands::Reset { .. } | Commands::Recover => {
            bail!("This command should not be dispatched through the client");
        }
    };

    let response = send_request(request).await?;
    print_response(&response);
    Ok(())
}
