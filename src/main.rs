use anyhow::Error;
use directories::ProjectDirs;
use mcp_linux_ssh::handler::POSIXSSHHandler;
use rust_mcp_sdk::{
    McpServer, StdioTransport, TransportOptions,
    mcp_server::{McpServerOptions, ToMcpServerHandler, server_runtime},
    schema::{
        Implementation, InitializeResult, LATEST_PROTOCOL_VERSION, ServerCapabilities,
        ServerCapabilitiesTools,
    },
};
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{
    EnvFilter, fmt, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

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
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .json()
                .with_ansi(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
        .init();

    tracing::info!("starting");

    // Define server details & capabilities
    let server_details = InitializeResult {
        server_info: Implementation {
            name: env!("CARGO_PKG_NAME").to_string(),
            title: Some("Linux SSH Administration".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: Some("A MCP server to run operations on a POSIX compatible system (Linux, BSD, macOS) machine over SSH".to_string()),
            icons: vec![],
            website_url: Some("https://github.com/subpop/mcp_linux_ssh".to_string()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        instructions: Some(String::from(
            "You are an expert POSIX compatible system (Linux, BSD, macOS) system \
            administrator. You run commands on a remote POSIX compatible system \
            (Linux, BSD, macOS) system to troubleshoot, fix issues and perform \
            general administration.",
        )),
        meta: None,
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    };

    // Create transport with default options
    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|e| Error::msg(format!("{}", e)))?;

    // Create custom handler with judge initialization
    let handler = POSIXSSHHandler::new().await;
    let handler_arc: Arc<dyn rust_mcp_sdk::mcp_server::McpServerHandler> =
        handler.to_mcp_server_handler();

    // Create server options
    let server_options = McpServerOptions {
        server_details,
        transport,
        handler: handler_arc,
        task_store: None,
        client_task_store: None,
    };

    // Create Server
    let server = server_runtime::create_server(server_options);

    // Start!
    server
        .start()
        .await
        .map_err(|e| Error::msg(format!("{}", e)))
}
