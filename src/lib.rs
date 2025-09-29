use anyhow::Result;
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
use std::process::Command;
use url::Url;

/// Handler for the MCP server.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[derive(Clone, Deserialize, JsonSchema)]
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
        description = "Run a command on a remote POSIX compatible system (Linux, BSD, macOS) system and return the output."
    )]
    pub async fn run_command_ssh(
        &self,
        params: Parameters<RunCommandSshParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = params.0;
        let command = params.command;
        let args = params.args;
        let remote_user = params.remote_user.unwrap_or(whoami::username());
        let remote_host = params.remote_host;

        match run_ssh_command(
            &remote_user,
            &remote_host,
            &command,
            &args.iter().map(|arg| arg.as_str()).collect::<Vec<&str>>(),
        ) {
            Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
            Err(e) => Err(e),
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation::from_build_env(),
            instructions: Some(String::from(
                "You are an expert POSIX compatible system (Linux, BSD, macOS) system administrator. You run commands on a remote POSIX compatible system (Linux, BSD, macOS) system to troubleshoot, fix issues and perform general administration.",
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

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        // Parse the URI into a URL struct
        let url = Url::parse(&request.uri)
            .map_err(|e| ErrorData::invalid_request(format!("Invalid URI: {}", e), None))?;

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
                        ErrorData::invalid_request(
                            format!("Invalid percent-encoding in path: {}", e),
                            None,
                        )
                    })?
                    .to_string();

                // Use SSH to read the file contents
                match run_ssh_command(&user, host, "cat", &[&file_path]) {
                    Ok(output) => Ok(ReadResourceResult {
                        contents: vec![ResourceContents::text(output, request.uri)],
                    }),
                    Err(e) => Err(e),
                }
            }
            _ => {
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
fn run_ssh_command(
    user: &str,
    host: &str,
    command: &str,
    args: &[&str],
) -> Result<String, ErrorData> {
    let output = Command::new("ssh")
        .arg(host)
        .args(["-l", user])
        .arg(command)
        .args(args)
        .output()
        .map_err(|e| ErrorData::internal_error(format!("Failed to run command: {}", e), None))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrorData::internal_error(
            format!("Failed to run command: {}", stderr),
            None,
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
