use crate::config::FailMode;
use anyhow::{Context, Result};
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use serde::{Deserialize, Serialize};
use siumai::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout as tokio_timeout;
use tracing::{error, warn};

/// System prompt for the judge (hardcoded)
const SYSTEM_PROMPT: &str = "You are a security judge for an SSH command execution system. \
Evaluate tool calls for safety and appropriateness. \
Consider: \
- Destructive operations (rm, format, dd, etc.) \
- Sensitive system paths (/etc, /root, /boot, etc.) \
- Production vs development environments \
- Network operations that could affect connectivity \
- File operations that could overwrite critical files \
\n\
Return JSON: {\"allowed\": true/false, \"reason\": \"brief explanation\"}";

/// Result of a judgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResult {
    pub allowed: bool,
    pub reason: String,
}

/// Judge service that evaluates tool calls using an LLM
#[derive(Clone, Serialize)]
pub struct JudgeService {
    #[serde(skip)]
    client: Arc<dyn ChatCapability + Send + Sync>,
    fail_mode: FailMode,
    judge_tools: HashSet<String>,
    system_prompt: String,
    timeout: Duration,
}

impl std::fmt::Debug for JudgeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JudgeService")
            .field("fail_mode", &self.fail_mode)
            .field("judge_tools", &self.judge_tools)
            .field("system_prompt", &self.system_prompt)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl JudgeService {
    /// Create a new judge service from configuration
    pub async fn from_config(config: crate::config::JudgeConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout_seconds);

        // Parse judge tools from comma-separated string
        let judge_tools: HashSet<String> = config
            .tools
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Build LLM client based on provider type
        let client: Arc<dyn ChatCapability + Send + Sync> = match config.service.as_str() {
            "openai" => {
                if config.api_key.is_empty() {
                    anyhow::bail!("MCP_LINUX_SSH_JUDGE_API_KEY is required for OpenAI");
                }
                let mut builder = Siumai::builder()
                    .openai()
                    .api_key(&config.api_key)
                    .model(&config.model);

                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                }

                Arc::new(
                    builder
                        .build()
                        .await
                        .context("Failed to create OpenAI client")?,
                )
            }
            "anthropic" => {
                if config.api_key.is_empty() {
                    anyhow::bail!("MCP_LINUX_SSH_JUDGE_API_KEY is required for Anthropic");
                }
                let mut builder = Siumai::builder()
                    .anthropic()
                    .api_key(&config.api_key)
                    .model(&config.model);

                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                }

                Arc::new(
                    builder
                        .build()
                        .await
                        .context("Failed to create Anthropic client")?,
                )
            }
            "ollama" => {
                let mut builder = Siumai::builder().ollama().model(&config.model);

                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                } else {
                    builder = builder.base_url("http://localhost:11434");
                }

                Arc::new(
                    builder
                        .build()
                        .await
                        .context("Failed to create Ollama client")?,
                )
            }
            "gemini" => {
                if config.api_key.is_empty() {
                    anyhow::bail!("MCP_LINUX_SSH_JUDGE_API_KEY is required for Gemini");
                }
                let mut builder = Siumai::builder()
                    .gemini()
                    .api_key(&config.api_key)
                    .model(&config.model);

                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                }

                Arc::new(
                    builder
                        .build()
                        .await
                        .context("Failed to create Gemini client")?,
                )
            }
            _ => {
                anyhow::bail!(
                    "Unsupported provider type: {}. Supported: openai, anthropic, ollama, gemini",
                    config.service
                );
            }
        };

        Ok(Self {
            client,
            fail_mode: config.fail_mode,
            judge_tools,
            system_prompt: SYSTEM_PROMPT.to_string(),
            timeout,
        })
    }

    /// Check if a tool should be judged
    pub fn should_judge(&self, tool_name: &str) -> bool {
        self.judge_tools.contains(tool_name)
    }

    /// Judge a tool call and return an error if rejected
    pub async fn check_tool_call(
        &self,
        tool_name: &str,
        tool_params: &serde_json::Value,
    ) -> Result<(), CallToolError> {
        // Build the prompt
        let prompt = format!(
            "Tool: {}\nParameters:\n{}\n\nEvaluate if this tool call should be allowed. Return JSON: {{\"allowed\": true/false, \"reason\": \"brief explanation\"}}",
            tool_name,
            serde_json::to_string_pretty(tool_params)
                .unwrap_or_else(|_| format!("{:?}", tool_params))
        );

        // Create the messages
        let messages = vec![system!(&self.system_prompt), user!(&prompt)];

        // Execute with timeout
        let result = tokio_timeout(self.timeout, async { self.client.chat(messages).await }).await;

        let response = match result {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                error!("LLM execution error: {}", e);
                return handle_llm_error(&self.fail_mode, "LLM execution failed");
            }
            Err(_) => {
                warn!("LLM judge timeout after {:?}", self.timeout);
                return handle_llm_error(&self.fail_mode, "LLM judge timeout");
            }
        };

        // Get response text
        let response_text = response
            .content_text()
            .map(|s| s.to_string())
            .unwrap_or_else(|| response.content.all_text());

        // Parse the JSON response
        let judgment: JudgeResult = match serde_json::from_str(&response_text) {
            Ok(j) => j,
            Err(e) => {
                // Try to extract JSON from the response if it's embedded in text
                if let Some(json_start) = response_text.find('{') {
                    if let Some(json_end) = response_text.rfind('}') {
                        let json_str = &response_text[json_start..=json_end];
                        match serde_json::from_str(json_str) {
                            Ok(j) => j,
                            Err(_) => {
                                error!("Failed to parse LLM response as JSON: {}", e);
                                return handle_llm_error(
                                    &self.fail_mode,
                                    "Failed to parse judge response",
                                );
                            }
                        }
                    } else {
                        error!("Failed to parse LLM response as JSON: {}", e);
                        return handle_llm_error(&self.fail_mode, "Failed to parse judge response");
                    }
                } else {
                    error!("Failed to parse LLM response as JSON: {}", e);
                    return handle_llm_error(&self.fail_mode, "Failed to parse judge response");
                }
            }
        };

        // Check the judgment
        if !judgment.allowed {
            return Err(CallToolError::from_message(format!(
                "Tool call rejected by judge: {}",
                judgment.reason
            )));
        }

        Ok(())
    }
}

/// Handle LLM errors based on fail mode
fn handle_llm_error(fail_mode: &FailMode, message: &str) -> Result<(), CallToolError> {
    match fail_mode {
        FailMode::Closed => Err(CallToolError::from_message(format!(
            "Judge unavailable: {}",
            message
        ))),
        FailMode::Open => {
            warn!(
                "Judge unavailable (fail_mode=open), allowing tool call: {}",
                message
            );
            Ok(())
        }
    }
}
