use anyhow::Result;
use rmcp::ErrorData;
use rmcp::RoleServer;
use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, GetPromptRequestParam, GetPromptResult, Implementation,
    ListPromptsResult, PaginatedRequestParam, ServerCapabilities, ServerInfo,
};
use rmcp::prompt_handler;
use rmcp::prompt_router;
use rmcp::service::RequestContext;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use schemars::JsonSchema;
use serde::Deserialize;
use std::process::Command;

/// Handler for the MCP server.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct RunCommandSshParams {
    /// The command to run.
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

    #[tool(description = "Run a command on a remote Linux system and return the output.")]
    pub async fn run_command_ssh(
        &self,
        params: Parameters<RunCommandSshParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = params.0;
        let command = params.command;
        let args = params.args;
        let remote_user = params.remote_user.unwrap_or(whoami::username());
        let remote_host = params.remote_host;

        let output = Command::new("ssh")
            .arg(remote_host)
            .args(["-l", &remote_user])
            .arg(command)
            .args(args)
            .output()
            .map_err(|e| {
                ErrorData::internal_error(format!("Failed to run command: {}", e), None)
            })?;

        Ok(CallToolResult::success(vec![Content::text(
            String::from_utf8_lossy(&output.stdout),
        )]))
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation::from_build_env(),
            instructions: Some(String::from(
                "You are an expert Linux system administrator. You run commands on a remote Linux system to troubleshoot, fix issues and perform general administration.",
            )),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
            ..Default::default()
        }
    }
}
