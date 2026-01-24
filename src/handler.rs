use async_trait::async_trait;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, RpcError,
};
use rust_mcp_sdk::{McpServer, mcp_server::ServerHandler};
use std::sync::Arc;

use crate::judge::JudgeService;
use crate::tools::POSIXSSHTools;

pub struct POSIXSSHHandler {
    judge_service: Option<Arc<JudgeService>>,
}

impl POSIXSSHHandler {
    pub async fn new() -> Self {
        let judge_service = match crate::config::load_judge_service().await {
            Ok(Some(service)) => Some(service),
            Ok(None) => {
                tracing::debug!("Judge service not configured, judge disabled");
                None
            }
            Err(e) => {
                tracing::warn!("Failed to load judge service: {}", e);
                None
            }
        };

        Self { judge_service }
    }

    /// Check if a tool call should be judged and validate it
    async fn check_tool_call(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> Result<(), CallToolError> {
        if let Some(judge) = &self.judge_service
            && judge.should_judge(tool_name)
        {
            judge.check_tool_call(tool_name, params).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl ServerHandler for POSIXSSHHandler {
    /// Handle list tool requests
    async fn handle_list_tools_request(
        &self,
        _: Option<PaginatedRequestParams>,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: POSIXSSHTools::tools(),
        })
    }

    /// Handle tool call requests
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        // Get tool name from params
        let tool_name = params.name.as_str();

        // Get parameters as JSON for judge
        let tool_params_json =
            serde_json::Value::Object(params.arguments.clone().unwrap_or_default());

        // Check with judge before executing
        self.check_tool_call(tool_name, &tool_params_json).await?;

        // Convert to tool enum and execute
        let tool_params = POSIXSSHTools::try_from(params).map_err(CallToolError::new)?;

        match tool_params {
            POSIXSSHTools::RunLocalCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::RunSSHCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::RunSSHSudoCommand(tool) => tool.call_tool().await,
            POSIXSSHTools::CopyFile(tool) => tool.call_tool().await,
            POSIXSSHTools::PatchFile(tool) => tool.call_tool().await,
        }
    }
}
