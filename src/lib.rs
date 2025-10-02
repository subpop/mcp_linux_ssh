use anyhow::Result;
use expand_tilde::expand_tilde;
use rmcp::{
    ErrorData, RoleServer,
    handler::server::{
        ServerHandler, router::prompt::PromptRouter, tool::ToolRouter, wrapper::Parameters,
    },
    model::{
        CallToolResult, GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParam, ServerCapabilities, ServerInfo,
    },
    prompt_handler, prompt_router,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::ops::Deref;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

/// Handler for the MCP server.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RunCommandSshParams {
    /// The command to run. This must be a single command. Arguments must be
    /// passed in the args parameter.
    pub command: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to run the command on.
    pub remote_host: String,
    /// Path to the private key to use for authentication. Defaults to \
    /// ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds.
    /// Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RunCommandLocalParams {
    /// The command to run. This must be a single command. Arguments must be
    /// passed in the args parameter.
    pub command: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds.
    /// Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
}

#[tool_router]
#[prompt_router]
impl Handler {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    #[tool(
        name = "Run",
        description = "Run a command on the local system and return the output. \
        Use this sparingly; only when needed to troubleshoot why connecting to the \
        remote system is failing."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn run_command_local(
        &self,
        params: Parameters<RunCommandLocalParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::DEBUG, "run_command_local", params = ?params);
        let _enter = _span.enter();

        let command = params.0.command;
        let args = params.0.args;
        let timeout_seconds = params.0.timeout_seconds.unwrap_or(30);

        let command_future = Command::new(&command).args(&args).output();

        let result = if timeout_seconds == 0 {
            // No timeout - run indefinitely
            command_future.await
        } else {
            // Apply timeout
            let timeout_duration = Duration::from_secs(timeout_seconds);
            match timeout(timeout_duration, command_future).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(ErrorData::internal_error(
                        format!("Local command timed out after {} seconds", timeout_seconds),
                        None,
                    ));
                }
            }
        };

        match result {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status_code = output.status.code();

                Ok(CallToolResult::structured(json!({
                    "status_code": status_code,
                    "stdout": stdout.trim().to_string(),
                    "stderr": stderr.trim().to_string(),
                })))
            }
            Err(e) => Err(ErrorData::internal_error(
                // The command failed to execute. Return the error to the caller.
                format!("Failed to execute local command: {}", e),
                None,
            )),
        }
    }

    #[tool(
        name = "SSH",
        description = "Run a command on a remote POSIX compatible system (Linux, \
        BSD, macOS) system and return the output. This tool does not permit commands \
        to be run with sudo."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn run_command_ssh(
        &self,
        params: Parameters<RunCommandSshParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_command_ssh", params = ?params);
        let _enter = _span.enter();

        let command = params.0.command;
        let args = params.0.args;
        let remote_user = params.0.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.remote_host;
        let private_key = params
            .0
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.timeout_seconds.unwrap_or(30);

        if command.contains("sudo") || args.iter().any(|arg| arg.contains("sudo")) {
            // sudo is not permitted for this tool.
            return Err(ErrorData::invalid_request(
                "You many not run commands with sudo using this tool".to_string(),
                None,
            ));
        }

        match exec_ssh(
            &remote_user,
            &remote_host,
            &private_key,
            &command,
            &args.iter().map(|arg| arg.as_str()).collect::<Vec<&str>>(),
            timeout_seconds,
        )
        .await
        {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status_code = output.status.code();

                Ok(CallToolResult::structured(json!({
                    "status_code": status_code,
                    "stdout": stdout.trim().to_string(),
                    "stderr": stderr.trim().to_string(),
                })))
            }
            Err(e) => Err(ErrorData::internal_error(
                // The command failed to execute. Return the error to the caller.
                format!("Failed to execute remote SSH command: {}", e),
                None,
            )),
        }
    }

    #[tool(
        name = "SSH_Sudo",
        description = "Run a command on a remote POSIX compatible system (Linux, \
        BSD, macOS) system and return the output. This tool explicitly runs \
        commands with sudo."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn run_command_ssh_sudo(
        &self,
        params: Parameters<RunCommandSshParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_command_ssh_sudo", params = ?params);
        let _enter = _span.enter();

        let command = params.0.command;
        let args = params.0.args;
        let remote_user = params.0.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.remote_host;
        let private_key = params
            .0
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.timeout_seconds.unwrap_or(30);

        match exec_ssh(
            &remote_user,
            &remote_host,
            &private_key,
            "sudo",
            std::iter::once(command.as_str())
                .chain(args.iter().map(|arg| arg.as_str()))
                .collect::<Vec<&str>>()
                .as_slice(),
            timeout_seconds,
        )
        .await
        {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status_code = output.status.code();

                Ok(CallToolResult::structured(json!({
                    "status_code": status_code,
                    "stdout": stdout.trim().to_string(),
                    "stderr": stderr.trim().to_string(),
                })))
            }
            Err(e) => Err(ErrorData::internal_error(
                // The command failed to execute. Return the error to the caller.
                format!("Failed to execute remote SSH command with sudo: {}", e),
                None,
            )),
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: String::from("Linux admin utilities"),
                title: Some(String::from("POSIX administration using SSH")),
                ..Default::default()
            },
            instructions: Some(String::from(
                "You are an expert POSIX compatible system (Linux, BSD, macOS) system \
                administrator. You run commands on a remote POSIX compatible system \
                (Linux, BSD, macOS) system to troubleshoot, fix issues and perform \
                general administration.",
            )),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }
}

/// Run a command on a remote POSIX compatible system (Linux, BSD, macOS) system
/// via SSH.
#[tracing::instrument]
async fn exec_ssh(
    user: &str,
    host: &str,
    private_key: &str,
    command: &str,
    args: &[&str],
    timeout_seconds: u64,
) -> Result<std::process::Output, ErrorData> {
    let _span = tracing::span!(tracing::Level::TRACE, "exec_ssh", user = %user, host = %host, private_key = %private_key, command = %command, args = ?args, timeout_seconds = %timeout_seconds);
    let _enter = _span.enter();

    let expanded_key = expand_tilde(private_key).map_err(|e| {
        ErrorData::internal_error(format!("Failed to expand private key path: {}", e), None)
    })?;
    let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
        ErrorData::internal_error(
            format!("Failed to convert private key to string: {}", private_key),
            None,
        )
    })?;

    let command_future = Command::new("ssh")
        .arg(host)
        .args(["-l", user])
        .args(["-i", private_key_path])
        .arg(command)
        .args(args)
        .output();

    let result = if timeout_seconds == 0 {
        // No timeout - run indefinitely
        command_future.await
    } else {
        // Apply timeout
        let timeout_duration = Duration::from_secs(timeout_seconds);
        match timeout(timeout_duration, command_future).await {
            Ok(result) => result,
            Err(_) => {
                return Err(ErrorData::internal_error(
                    format!("SSH command timed out after {} seconds", timeout_seconds),
                    None,
                ));
            }
        }
    };

    result.map_err(|e| ErrorData::internal_error(format!("Failed to run SSH command: {}", e), None))
}
