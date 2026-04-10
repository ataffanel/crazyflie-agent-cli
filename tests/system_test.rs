//! System tests for crazyflie-agent-cli.
//!
//! These tests require a real Crazyflie connected via Crazyradio.
//! Set CRAZYFLIE_URI to run them:
//!
//!   CRAZYFLIE_URI=radio://0/80/2M/E7E7E7E7E7 cargo test -- --test-threads=1
//!
//! Tests are ignored when CRAZYFLIE_URI is not set.
//! They MUST run sequentially (--test-threads=1) because they share one radio
//! and one daemon process.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn cli() -> String {
    // Use the binary built by cargo
    env!("CARGO_BIN_EXE_crazyflie-agent-cli").to_string()
}

fn uri() -> Option<String> {
    std::env::var("CRAZYFLIE_URI").ok()
}

/// Run the CLI with the given args and return (stdout, stderr, success).
fn run(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(cli())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to execute CLI");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

/// Run the CLI with a timeout. Returns None if timed out.
fn run_with_timeout(args: &[&str], timeout: Duration) -> Option<(String, String, bool)> {
    let mut child = Command::new(cli())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute CLI");

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map(|s| {
                    use std::io::Read;
                    let mut buf = String::new();
                    let mut r = s;
                    r.read_to_string(&mut buf).ok();
                    buf
                }).unwrap_or_default();
                let stderr = child.stderr.take().map(|s| {
                    use std::io::Read;
                    let mut buf = String::new();
                    let mut r = s;
                    r.read_to_string(&mut buf).ok();
                    buf
                }).unwrap_or_default();
                return Some((stdout, stderr, status.success()));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    child.kill().ok();
                    child.wait().ok();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// Start the daemon in the background. Returns the child process.
fn start_daemon(uri: &str) -> std::process::Child {
    Command::new(cli())
        .args(["start", uri])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start daemon")
}

/// Wait until `status` returns connected, or panic after timeout.
fn wait_for_daemon(timeout: Duration) {
    let start = Instant::now();
    loop {
        let (stdout, _, ok) = run(&["status"]);
        if ok && stdout.contains("connected: true") {
            return;
        }
        if start.elapsed() > timeout {
            panic!("daemon did not become ready within {:?}", timeout);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Stop the daemon if it's running, and wait for it to exit.
fn stop_daemon(mut child: std::process::Child) {
    let _ = run(&["stop"]);
    // Give it a moment to shut down
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(5) {
                    child.kill().ok();
                    child.wait().ok();
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

// ============================================================
// Tests
// ============================================================

/// Guard that starts the daemon before tests and stops it after,
/// even on panic.
struct DaemonGuard {
    child: Option<std::process::Child>,
}

impl DaemonGuard {
    fn start(uri: &str) -> Self {
        let child = start_daemon(uri);
        wait_for_daemon(Duration::from_secs(15));
        Self { child: Some(child) }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            stop_daemon(child);
        }
    }
}

#[test]
fn test_scan() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };

    let (stdout, _, ok) = run(&["scan"]);
    assert!(ok, "scan should succeed");
    assert!(
        stdout.contains(&uri),
        "scan output should contain the expected URI.\nGot: {}",
        stdout
    );
}

#[test]
fn test_start_status_stop() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };

    let daemon = DaemonGuard::start(&uri);

    // status should report connected
    let (stdout, _, ok) = run(&["status"]);
    assert!(ok, "status should succeed");
    assert!(stdout.contains("connected: true"), "should be connected.\nGot: {}", stdout);
    assert!(stdout.contains(&uri), "status should contain URI.\nGot: {}", stdout);
    assert!(stdout.contains("firmware:"), "status should contain firmware version.\nGot: {}", stdout);

    // stop
    let (stdout, _, ok) = run(&["stop"]);
    assert!(ok, "stop should succeed");
    assert!(stdout.contains("stopping"), "stop should confirm.\nGot: {}", stdout);

    // Wait for daemon to exit
    drop(daemon);

    // status should now fail
    let (_, _, ok) = run(&["status"]);
    assert!(!ok, "status should fail after stop");
}

#[test]
fn test_param_list() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    let (stdout, _, ok) = run(&["param", "list"]);
    assert!(ok, "param list should succeed");

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() > 10, "should list many params, got {}", lines.len());

    // Each line should have 4 tab-separated fields: name, type, access, value
    for line in &lines[..5] {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "expected 4 fields, got {:?}", fields);
    }
}

#[test]
fn test_param_get() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    // Read a well-known parameter
    let (stdout, _, ok) = run(&["param", "get", "system.selftestPassed"]);
    assert!(ok, "param get should succeed");
    let value = stdout.trim();
    assert!(!value.is_empty(), "param value should not be empty");
}

#[test]
fn test_param_set_and_get() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    // Read original value
    let (original, _, ok) = run(&["param", "get", "sound.effect"]);
    assert!(ok, "param get should succeed");
    let original = original.trim().to_string();

    // Set to a different value
    let new_value = if original == "0" { "1" } else { "0" };
    let (stdout, _, ok) = run(&["param", "set", "sound.effect", new_value]);
    assert!(ok, "param set should succeed");
    assert!(stdout.contains(new_value), "should confirm new value.\nGot: {}", stdout);

    // Read back
    let (readback, _, ok) = run(&["param", "get", "sound.effect"]);
    assert!(ok, "param get should succeed after set");
    assert_eq!(readback.trim(), new_value, "readback should match set value");

    // Restore original
    let (_, _, ok) = run(&["param", "set", "sound.effect", &original]);
    assert!(ok, "restoring original value should succeed");
}

#[test]
fn test_param_get_nonexistent() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    let (_, stderr, ok) = run(&["param", "get", "nonexistent.param"]);
    assert!(!ok, "param get of nonexistent should fail");
    assert!(stderr.contains("error"), "should print error.\nGot: {}", stderr);
}

#[test]
fn test_param_set_wrong_type() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    let (_, stderr, ok) = run(&["param", "set", "sound.effect", "not_a_number"]);
    assert!(!ok, "param set with wrong type should fail");
    assert!(stderr.contains("error"), "should print error.\nGot: {}", stderr);
}

#[test]
fn test_param_set_readonly() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    // Setting a read-only parameter should fail with an error, not hang.
    let result = run_with_timeout(
        &["param", "set", "firmware.revision0", "42"],
        Duration::from_secs(5),
    );
    match result {
        None => panic!("param set on read-only parameter timed out (hung for >5s)"),
        Some((_, stderr, ok)) => {
            assert!(!ok, "param set on read-only should fail");
            assert!(
                stderr.contains("error") || stderr.contains("read"),
                "should report an error about read-only.\nstderr: {}",
                stderr
            );
        }
    }
}

#[test]
fn test_log_list() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    let (stdout, _, ok) = run(&["log", "list"]);
    assert!(ok, "log list should succeed");

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() > 10, "should list many log vars, got {}", lines.len());

    // Each line should have 2 tab-separated fields: name, type
    for line in &lines[..5] {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 2, "expected 2 fields, got {:?}", fields);
    }
}

#[test]
fn test_log_start_invalid_variable() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };
    let _daemon = DaemonGuard::start(&uri);

    let (_, stderr, ok) = run(&["log", "start", "nonexistent.var", "--rate", "1"]);
    assert!(!ok, "log start with invalid var should fail");
    assert!(stderr.contains("error"), "should print error.\nGot: {}", stderr);
}

#[test]
fn test_log_start_and_stop() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };

    // Start daemon and capture its stdout via a pipe so we can check log lines
    let daemon = Command::new(cli())
        .args(["start", &uri])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start daemon");

    wait_for_daemon(Duration::from_secs(15));

    // Start logging at 10 Hz
    let (stdout, _, ok) = run(&["log", "start", "pm.vbat", "--rate", "10"]);
    assert!(ok, "log start should succeed");
    assert!(stdout.contains("log started"), "should confirm.\nGot: {}", stdout);

    // Wait for some data to arrive
    std::thread::sleep(Duration::from_secs(2));

    // Stop logging
    let (stdout, _, ok) = run(&["log", "stop"]);
    assert!(ok, "log stop should succeed");
    assert!(stdout.contains("log stopped"), "should confirm.\nGot: {}", stdout);

    // Wait and then check that no more data is arriving.
    // We do this by stopping the daemon and reading its stdout.
    std::thread::sleep(Duration::from_secs(2));

    // Stop daemon
    let _ = run(&["stop"]);
    let output = daemon.wait_with_output().expect("failed to read daemon output");
    let daemon_stdout = String::from_utf8_lossy(&output.stdout);

    // Count log lines before and after the stop.
    // All [log ...] lines should have timestamps. Split by the log stop point.
    let log_lines: Vec<&str> = daemon_stdout.lines().filter(|l| l.starts_with("[log")).collect();
    assert!(!log_lines.is_empty(), "should have received some log data");

    // After log stop, there should be no more log lines arriving.
    // We can verify this by checking that log data stopped within a reasonable
    // window after log stop was issued.
    //
    // Since we waited 2 seconds after stop at 10 Hz, if logging didn't actually
    // stop we'd see ~20 extra lines. If it stopped properly we'd see 0.
    //
    // We slept 2s before stop and 2s after: roughly half the lines should be
    // before and none after if stop works. With 10 Hz and 2s we expect ~20 lines
    // before stop. If we got >>20 lines total, logging leaked past the stop.
    let expected_before_stop = 20; // ~10Hz * 2s
    let tolerance = 10; // some slack for timing

    assert!(
        log_lines.len() <= expected_before_stop + tolerance,
        "log stop did not actually stop logging: got {} log lines \
         (expected ~{} before stop, but data kept flowing after stop)",
        log_lines.len(),
        expected_before_stop
    );
}

#[test]
fn test_log_start_replaces_previous() {
    let uri = match uri() {
        Some(u) => u,
        None => { eprintln!("CRAZYFLIE_URI not set, skipping"); return; }
    };

    let daemon = Command::new(cli())
        .args(["start", &uri])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start daemon");

    wait_for_daemon(Duration::from_secs(15));

    // Start logging pm.vbat
    let (_, _, ok) = run(&["log", "start", "pm.vbat", "--rate", "5"]);
    assert!(ok, "first log start should succeed");
    std::thread::sleep(Duration::from_secs(2));

    // Start logging a different variable - should REPLACE the previous block
    let (_, _, ok) = run(&["log", "start", "stateEstimate.yaw", "--rate", "5"]);
    assert!(ok, "second log start should succeed");
    std::thread::sleep(Duration::from_secs(2));

    // Stop daemon and read output
    let _ = run(&["stop"]);
    let output = daemon.wait_with_output().expect("failed to read daemon output");
    let daemon_stdout = String::from_utf8_lossy(&output.stdout);

    // After the second log start, only stateEstimate.yaw should appear.
    // If pm.vbat still appears after the second log start, the old block leaked.
    let lines: Vec<&str> = daemon_stdout.lines().collect();

    // Find the index where stateEstimate.yaw first appears
    let yaw_start = lines.iter().position(|l| l.contains("stateEstimate.yaw"));
    assert!(yaw_start.is_some(), "should have yaw log lines");
    let yaw_start = yaw_start.unwrap();

    // After that point, pm.vbat should NOT appear
    let vbat_after_yaw: Vec<&&str> = lines[yaw_start..]
        .iter()
        .filter(|l| l.contains("pm.vbat"))
        .collect();

    assert!(
        vbat_after_yaw.is_empty(),
        "after starting a new log block, the old one (pm.vbat) should stop.\n\
         Found {} pm.vbat lines after yaw started streaming.\n\
         This means log start does not replace the previous log block.",
        vbat_after_yaw.len()
    );
}
