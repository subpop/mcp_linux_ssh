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
use std::{ops::Deref, process::Command};

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
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RunCommandLocalParams {
    /// The command to run. This must be a single command. Arguments must be
    /// passed in the args parameter.
    pub command: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
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

        match Command::new(&command).args(&args).output() {
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
        ) {
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

        match exec_ssh(
            &remote_user,
            &remote_host,
            &private_key,
            &command,
            &args.iter().map(|arg| arg.as_str()).collect::<Vec<&str>>(),
        ) {
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
fn exec_ssh(
    user: &str,
    host: &str,
    private_key: &str,
    command: &str,
    args: &[&str],
) -> Result<std::process::Output, ErrorData> {
    let _span = tracing::span!(tracing::Level::TRACE, "exec_ssh", user = %user, host = %host, private_key = %private_key, command = %command, args = ?args);
    let _enter = _span.enter();

    let output = Command::new("ssh")
        .arg(host)
        .args(["-l", user])
        .args([
            "-i",
            expand_tilde(private_key)
                .map_err(|e| {
                    ErrorData::internal_error(
                        format!("Failed to expand private key path: {}", e),
                        None,
                    )
                })?
                .deref()
                .as_os_str()
                .to_str()
                .ok_or_else(|| {
                    ErrorData::internal_error(
                        format!("Failed to convert private key to string: {}", private_key),
                        None,
                    )
                })?,
        ])
        .arg(command)
        .args(args)
        .output()
        .map_err(|e| ErrorData::internal_error(format!("Failed to run command: {}", e), None))?;

    Ok(output)
}
