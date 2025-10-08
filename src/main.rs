use anyhow::Error;
use directories::ProjectDirs;
use mcp_linux_ssh::Handler;
use rmcp::{ServiceExt, transport::stdio};
use std::fs::create_dir_all;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing with file and stderr logging.

    // Determine the state directory according to platform conventions and create it if it
    // doesn't exist.
    let project_dirs = ProjectDirs::from("net", "sub-pop", "mcp_linux_ssh")
        .ok_or_else(|| Error::msg("Failed to determine project directories"))?;
    let log_file_path: PathBuf = match project_dirs.state_dir() {
        Some(state_dir) => state_dir.join("tool_calls.jsonl"),
        None => {
            // Fall back to manually determined user directories.
            let user_dirs =
                directories::UserDirs::new().expect("Failed to determine user directories");
            user_dirs
                .home_dir()
                .join(".local")
                .join("state")
                .join("mcp_linux_ssh")
                .join("tool_calls.jsonl")
        }
    };
    let log_parent = log_file_path.parent().unwrap();
    create_dir_all(log_parent)
        .map_err(|e| Error::msg(format!("Failed to create log directory: {}", e)))?;

    let file_appender = tracing_appender::rolling::daily(log_parent, "tool_calls.jsonl");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .json()
                .with_ansi(false),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .json()
                .with_ansi(false),
        )
        .init();

    tracing::info!("starting");
    let handler = Handler::new();
    let server = handler
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("Error: {:?}", e))?;

    tracing::info!("started");
    server.waiting().await?;

    Ok(())
}
