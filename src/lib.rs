use anyhow::Result;
use expand_tilde::expand_tilde;
use rmcp::{
    ErrorData, RoleServer,
    handler::server::{
        ServerHandler, router::prompt::PromptRouter, tool::ToolRouter, wrapper::Parameters,
    },
    model::{
        CallToolResult, Content, GetPromptRequestParam, GetPromptResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, PaginatedRequestParam, RawResourceTemplate,
        ReadResourceRequestParam, ReadResourceResult, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerInfo,
    },
    prompt_handler, prompt_router,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{ops::Deref, process::Command};
use url::Url;

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
        let params = params.0;
        let command = params.command;
        let args = params.args;

        tracing::info!(command = %command, args = ?args, "executing local command");

        match Command::new(&command).args(&args).output() {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    tracing::info!(exit_code = ?output.status.code(), "local command succeeded");
                    Ok(CallToolResult::success(vec![Content::text(
                        stdout.trim().to_string(),
                    )]))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::error!(exit_code = ?output.status.code(), stderr = %stderr, "local command failed");
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "Command failed: {}",
                        stderr
                    ))]))
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to execute local command");
                Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error running command: {}",
                    e
                ))]))
            }
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
        let params = params.0;
        let command = params.command;
        let args = params.args;
        let remote_user = params.remote_user.unwrap_or(whoami::username());
        let remote_host = params.remote_host;
        let private_key = params
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());

        if command.starts_with("sudo") {
            return Err(ErrorData::invalid_request(
                "sudo is not permitted for this tool".to_string(),
                None,
            ));
        }

        tracing::info!(
            remote_user = %remote_user,
            remote_host = %remote_host,
            command = %command,
            args = ?args,
            "executing remote SSH command"
        );

        match exec_ssh(
            &remote_user,
            &remote_host,
            &private_key,
            &command,
            &args.iter().map(|arg| arg.as_str()).collect::<Vec<&str>>(),
        ) {
            Ok(output) => {
                tracing::info!("remote SSH command succeeded");
                Ok(CallToolResult::success(vec![Content::text(output)]))
            }
            Err(e) => {
                tracing::error!(error = %e, "remote SSH command failed");
                Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error running command: {}",
                    e
                ))]))
            }
        }
    }

    #[tool(
        name = "SSH_Sudo",
        description = "Run a command on a remote POSIX compatible system (Linux, \
        BSD, macOS) system and return the output. This tool permits commands to \
        be run with sudo."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn run_command_ssh_sudo(
        &self,
        params: Parameters<RunCommandSshParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = params.0;
        let command = params.command;
        let args = params.args;
        let remote_user = params.remote_user.unwrap_or(whoami::username());
        let remote_host = params.remote_host;
        let private_key = params
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());

        tracing::info!(
            remote_user = %remote_user,
            remote_host = %remote_host,
            command = %command,
            args = ?args,
            "executing remote SSH command"
        );

        match exec_ssh(
            &remote_user,
            &remote_host,
            &private_key,
            &command,
            &args.iter().map(|arg| arg.as_str()).collect::<Vec<&str>>(),
        ) {
            Ok(output) => {
                tracing::info!("remote SSH command succeeded");
                Ok(CallToolResult::success(vec![Content::text(output)]))
            }
            Err(e) => {
                tracing::error!(error = %e, "remote SSH command failed");
                Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error running command: {}",
                    e
                ))]))
            }
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
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![ResourceTemplate {
                raw: RawResourceTemplate {
                    title: Some("Remote File Contents".to_string()),
                    uri_template: "ssh://{user}@{host}/{path}".to_string(),
                    name: "Remote File Contents".to_string(),
                    description: Some(
                        "Access file contents on remote POSIX compatible system (Linux, BSD, macOS) systems via SSH".to_string(),
                    ),
                    mime_type: Some("text/plain".to_string()),
                },
                annotations: None,
            }],
            next_cursor: None,
        })
    }

    #[tracing::instrument(skip(self, _context))]
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        tracing::info!(uri = %request.uri, "reading resource");

        // Parse the URI into a URL struct
        let url = Url::parse(&request.uri).map_err(|e| {
            tracing::error!(error = %e, "invalid URI");
            ErrorData::invalid_request(format!("Invalid URI: {}", e), None)
        })?;

        match url.scheme() {
            "ssh" => {
                let user = if url.username().is_empty() {
                    whoami::username()
                } else {
                    url.username().to_string()
                };
                let host = url.host_str().unwrap();
                // Decode percent-encoded path to ensure file_path is not url-escape encoded
                let file_path = percent_encoding::percent_decode_str(url.path())
                    .decode_utf8()
                    .map_err(|e| {
                        tracing::error!(error = %e, "invalid percent-encoding in path");
                        ErrorData::invalid_request(
                            format!("Invalid percent-encoding in path: {}", e),
                            None,
                        )
                    })?
                    .to_string();

                tracing::info!(user = %user, host = %host, file_path = %file_path, "reading remote file via SSH");

                // Use SSH to read the file contents
                match exec_ssh(&user, host, "~/.ssh/id_ed25519", "cat", &[&file_path]) {
                    Ok(output) => {
                        tracing::info!("successfully read remote file");
                        Ok(ReadResourceResult {
                            contents: vec![ResourceContents::text(output, request.uri)],
                        })
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to read remote file");
                        Err(e)
                    }
                }
            }
            _ => {
                tracing::error!(scheme = %url.scheme(), "invalid URI scheme");
                return Err(ErrorData::invalid_request(
                    format!("Invalid URI scheme. Expected ssh://, got: {}", url.scheme()),
                    None,
                ));
            }
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
) -> Result<String, ErrorData> {
    tracing::debug!("spawning SSH process");

    let output = Command::new("ssh")
        .arg(host)
        .args(["-l", user])
        .args([
            "-i",
            expand_tilde(private_key)
                .map_err(|e| {
                    ErrorData::internal_error(format!("Failed to expand private key: {}", e), None)
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
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn SSH process");
            ErrorData::internal_error(format!("Failed to run command: {}", e), None)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(exit_code = ?output.status.code(), stderr = %stderr, "SSH command failed");
        return Err(ErrorData::internal_error(
            format!("Command executed unsuccessfully: {}", stderr),
            None,
        ));
    }

    tracing::debug!("SSH command completed successfully");
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
