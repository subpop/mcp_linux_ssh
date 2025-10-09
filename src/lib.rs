use anyhow::Result;
use expand_tilde::expand_tilde;
use rmcp::{
    ErrorData, RoleServer,
    handler::server::{
        ServerHandler, router::prompt::PromptRouter, tool::ToolRouter, wrapper::Parameters,
    },
    model::{
        AnnotateAble, CallToolResult, GetPromptRequestParam, GetPromptResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParam,
        RawResource, ReadResourceRequestParam, ReadResourceResult, ResourceContents,
        ServerCapabilities, ServerInfo,
    },
    prompt_handler, prompt_router,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::{fs, ops::Deref};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use url::Url;

/// Handler for the MCP server.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

/// Common SSH connection parameters shared across multiple tools.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct SshConnectionParams {
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to run the command on.
    pub remote_host: String,
    /// Path to the private key to use for authentication. Defaults to
    /// ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds.
    /// Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a
    /// key-value pair separated by an equal sign (=). The options are passed
    /// to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RunCommandSshParams {
    /// The command to run. This must be a single command. Arguments must be
    /// passed in the args parameter.
    pub command: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    #[serde(flatten)]
    #[schemars(flatten)]
    pub ssh: SshConnectionParams,
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

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CopyFileParams {
    /// The source file path on the local machine.
    pub source: String,
    /// The destination file path on the remote machine.
    pub destination: String,
    #[serde(flatten)]
    #[schemars(flatten)]
    pub ssh: SshConnectionParams,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct PatchFileParams {
    /// The patch/diff content to apply.
    pub patch: String,
    /// The path to the file on the remote machine to patch.
    pub remote_file: String,
    #[serde(flatten)]
    #[schemars(flatten)]
    pub ssh: SshConnectionParams,
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
        let remote_user = params.0.ssh.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.ssh.remote_host;
        let private_key = params
            .0
            .ssh
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.ssh.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = params
            .0
            .ssh
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

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
            options_vec.as_deref(),
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
        let remote_user = params.0.ssh.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.ssh.remote_host;
        let private_key = params
            .0
            .ssh
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.ssh.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = params
            .0
            .ssh
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

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
            options_vec.as_deref(),
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

    #[tool(
        name = "Copy_File",
        description = "Copy a file from the local machine to the remote machine using rsync. \
        Preserves file attributes and creates a backup if the destination file already exists."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn copy_file(
        &self,
        params: Parameters<CopyFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "copy_file", params = ?params);
        let _enter = _span.enter();

        let source = expand_tilde(&params.0.source).map_err(|e| {
            ErrorData::internal_error(format!("Failed to expand source path: {}", e), None)
        })?;
        let destination = params.0.destination;
        let remote_user = params.0.ssh.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.ssh.remote_host;
        let private_key = params
            .0
            .ssh
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.ssh.timeout_seconds.unwrap_or(30);

        // Expand the private key path
        let expanded_key = expand_tilde(&private_key).map_err(|e| {
            ErrorData::internal_error(format!("Failed to expand private key path: {}", e), None)
        })?;
        let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
            ErrorData::internal_error(
                format!("Failed to convert private key to string: {}", private_key),
                None,
            )
        })?;

        let ssh_command = format!("ssh -i {}", private_key_path);
        let remote_target = format!("{}@{}:{}", remote_user, remote_host, destination);

        // Build the rsync command
        // -a: archive mode (preserves permissions, timestamps, etc.)
        // -v: verbose
        // -b: create backups of existing files
        // -e: specify ssh command with identity file
        let command_future = Command::new("rsync")
            .arg("-avb")
            .arg("-e")
            .arg(&ssh_command)
            .arg(source.to_string_lossy().into_owned())
            .arg(&remote_target)
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
                        format!("rsync command timed out after {} seconds", timeout_seconds),
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
                format!("Failed to execute rsync command: {}", e),
                None,
            )),
        }
    }

    #[tool(
        name = "Patch_File",
        description = "Apply a patch or diff to a file on the remote machine using the patch command. \
        The patch content is streamed via stdin over SSH. By default, patch will attempt to \
        automatically detect the correct strip level (-p). Use unified diff format for best results."
    )]
    #[tracing::instrument(skip(self))]
    pub async fn patch_file(
        &self,
        params: Parameters<PatchFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "patch_file", params = ?params);
        let _enter = _span.enter();

        let patch = params.0.patch;
        let remote_file = params.0.remote_file;
        let remote_user = params.0.ssh.remote_user.unwrap_or(whoami::username());
        let remote_host = params.0.ssh.remote_host;
        let private_key = params
            .0
            .ssh
            .private_key
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = params.0.ssh.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = params
            .0
            .ssh
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        // Expand the private key path
        let expanded_key = expand_tilde(&private_key).map_err(|e| {
            ErrorData::internal_error(format!("Failed to expand private key path: {}", e), None)
        })?;
        let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
            ErrorData::internal_error(
                format!("Failed to convert private key to string: {}", private_key),
                None,
            )
        })?;

        // Build SSH command that will run patch on the remote side
        // The patch command reads from stdin and applies to the specified file
        let mut cmd = Command::new("ssh");
        cmd.arg(&remote_host)
            .args(["-l", &remote_user])
            .args(["-i", private_key_path])
            .args(
                options_vec
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|opt| ["-o", opt]),
            )
            .arg("patch")
            .arg(&remote_file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let command_future = async {
            let mut child = cmd.spawn().map_err(|e| {
                ErrorData::internal_error(format!("Failed to spawn SSH command: {}", e), None)
            })?;

            // Write the patch content to stdin
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(patch.as_bytes()).await.map_err(|e| {
                    ErrorData::internal_error(
                        format!("Failed to write patch to stdin: {}", e),
                        None,
                    )
                })?;
                // Close stdin to signal EOF
                drop(stdin);
            }

            // Wait for the command to complete
            child.wait_with_output().await.map_err(|e| {
                ErrorData::internal_error(format!("Failed to wait for SSH command: {}", e), None)
            })
        };

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
                        format!("Patch command timed out after {} seconds", timeout_seconds),
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
            Err(e) => Err(e),
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

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource {
                    uri: "file:///public_keys".to_string(),
                    name: "public_keys".to_string(),
                    title: Some("public_keys".to_string()),
                    description: Some("List public keys available on the local system".to_string()),
                    mime_type: Some("text/plain".to_string()),
                    size: None,
                    icons: None,
                }
                .no_annotation(),
            ],
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    #[tracing::instrument(skip(self, _context))]
    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let _span = tracing::span!(tracing::Level::TRACE, "read_resource", uri = %request.uri);
        let _enter = _span.enter();

        // Parse the URI into a URL struct
        let url = Url::parse(&request.uri)
            .map_err(|e| ErrorData::invalid_request(format!("Invalid URI: {}", e), None))?;

        match url.scheme() {
            "file" => {
                let path = url.to_file_path().map_err(|_| {
                    ErrorData::invalid_request("Invalid file URI".to_string(), None)
                })?;
                let path_str = path.to_str().ok_or_else(|| {
                    ErrorData::invalid_request("Cannot convert path to string", None)
                })?;

                match path_str {
                    "/public_keys" => {
                        // Find all public keys in ~/.ssh
                        let base_dirs = directories::BaseDirs::new().ok_or_else(|| {
                            ErrorData::internal_error("Cannot get home directory", None)
                        })?;
                        let home_dir = base_dirs.home_dir();
                        let ssh_dir = home_dir.join(".ssh");
                        let iter = fs::read_dir(&ssh_dir).map_err(|e| {
                            ErrorData::internal_error(
                                format!("Failed to read directory: {}", e),
                                None,
                            )
                        })?;

                        let mut public_keys = Vec::new();
                        for entry in iter {
                            let entry = entry.map_err(|e| {
                                ErrorData::internal_error(
                                    format!("Failed to read entry: {}", e),
                                    None,
                                )
                            })?;
                            let path = entry.path();
                            if path.is_file() && path.extension() == Some("pub".as_ref()) {
                                let basename = path
                                    .file_name()
                                    .ok_or_else(|| {
                                        ErrorData::internal_error(
                                            format!(
                                                "Cannot get base name for file {}",
                                                path.display()
                                            ),
                                            None,
                                        )
                                    })?
                                    .to_str()
                                    .ok_or_else(|| {
                                        ErrorData::internal_error(
                                            format!(
                                                "Cannot convert basename to string for file {}",
                                                path.display()
                                            ),
                                            None,
                                        )
                                    })?;

                                public_keys.push(String::from(basename));
                            }
                        }

                        Ok(ReadResourceResult {
                            contents: vec![ResourceContents::text(
                                public_keys.join(","),
                                request.uri.to_string(),
                            )],
                        })
                    }
                    _ => {
                        return Err(ErrorData::invalid_request(
                            format!("Invalid path: {}", path_str),
                            None,
                        ));
                    }
                }
            }
            _ => {
                return Err(ErrorData::invalid_request(
                    format!(
                        "Invalid URI scheme. Supported schemes: file://, got: {}",
                        url.scheme()
                    ),
                    None,
                ));
            }
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
    options: Option<&[&str]>,
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
        .args(
            options
                .unwrap_or_default()
                .iter()
                .flat_map(|opt| ["-o", opt]),
        )
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
