use clap::{Parser, Subcommand};

mod protocol;
mod output;
mod daemon;
mod client;
mod toc_cache;
mod commands;

#[derive(Parser, Debug)]
#[clap(name = "crazyflie-agent-cli", about = "CLI for AI agents to interact with Crazyflie drones")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start a persistent session (daemon) connected to a Crazyflie
    Start {
        /// Crazyflie URI (e.g. radio://0/80/2M/E7E7E7E7E7)
        uri: String,
    },
    /// Stop the running session
    Stop,
    /// Check if a session is running and the Crazyflie is connected
    Status,
    /// Scan for Crazyflies on the radio
    Scan,
    /// Parameter operations
    Param {
        #[clap(subcommand)]
        command: ParamCommands,
    },
    /// Log variable operations
    Log {
        #[clap(subcommand)]
        command: LogCommands,
    },
    /// Flash firmware to the Crazyflie
    Flash {
        /// Path to the firmware binary (.bin file)
        path: String,
        /// Crazyflie URI (e.g. radio://0/80/2M/E7E7E7E7E7). If omitted, scans for one.
        #[clap(long)]
        uri: Option<String>,
    },
    /// Reboot the Crazyflie
    Reset {
        /// Crazyflie URI. If omitted, scans for one.
        #[clap(long)]
        uri: Option<String>,
    },
    /// Recover a Crazyflie that can't be reached over radio
    Recover,
}

#[derive(Debug, Subcommand)]
pub enum ParamCommands {
    /// List all parameters
    List,
    /// Get a parameter value
    Get {
        /// Parameter name (e.g. pid_rate.kp_limit)
        name: String,
    },
    /// Set a parameter value
    Set {
        /// Parameter name
        name: String,
        /// Value to set
        value: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum LogCommands {
    /// List all log variables
    List,
    /// Start logging variables
    Start {
        /// Variable names to log
        variables: Vec<String>,
        /// Logging rate in Hz
        #[clap(long, default_value = "10")]
        rate: u64,
    },
    /// Stop all active log blocks
    Stop,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan => commands::scan::run().await,
        Commands::Start { uri } => daemon::run(&uri).await,
        Commands::Flash { path, uri } => commands::flash::run(&path, uri.as_deref()).await,
        Commands::Reset { uri } => commands::reset::run(uri.as_deref()).await,
        Commands::Recover => commands::recover::run().await,
        cmd => client::send_command(cmd).await,
    }
}
