use anyhow::Error;
use mcp_linux_ssh::Handler;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing with file and stderr logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
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
