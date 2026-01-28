use crate::judge::JudgeService;
use anyhow::Result;
use better_config::{EnvConfig, env};
use std::{str::FromStr, sync::Arc};
use tracing::info;

#[env(EnvConfig(prefix = "MCP_LINUX_SSH_JUDGE_", target = ""))]
pub struct JudgeConfig {
    #[conf(from = "SERVICE", default = "")]
    pub service: String,
    #[conf(from = "MODEL", default = "gpt-4o-mini")]
    pub model: String,
    #[conf(from = "API_KEY", default = "")]
    pub api_key: String,
    #[conf(from = "BASE_URL", default = "")]
    pub base_url: String,
    #[conf(from = "TIMEOUT_SECONDS", default = "10")]
    pub timeout_seconds: u64,
    #[conf(from = "FAIL_MODE", default = "open")]
    pub fail_mode: FailMode,
    #[conf(
        from = "TOOLS",
        default = "run_ssh_command,run_ssh_sudo_command,copy_file,patch_file,run_local_command"
    )]
    pub tools: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FailMode {
    Open,
    Closed,
}

impl FromStr for FailMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(FailMode::Open),
            "closed" => Ok(FailMode::Closed),
            _ => Err(anyhow::anyhow!("Invalid fail mode: {}", s)),
        }
    }
}

/// Load judge service from environment variables
pub async fn load_judge_service() -> Result<Option<Arc<JudgeService>>> {
    // Load configuration from environment variables using builder
    // Note: With target = "", we skip .env file loading and only use environment variables
    // The builder will load values directly from the process environment variables
    let config = match JudgeConfig::builder().build() {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!("Failed to load judge configuration: {}", e);
            return Ok(None);
        }
    };

    // Check if judge is enabled (service must not be empty)
    if config.service.is_empty() {
        tracing::debug!("Judge service not configured, judge disabled");
        return Ok(None);
    }

    info!("Initializing LLM judge service: {}", config.service);

    match JudgeService::from_config(config).await {
        Ok(service) => {
            info!("LLM judge service initialized successfully");
            Ok(Some(Arc::new(service)))
        }
        Err(e) => {
            tracing::warn!("Failed to initialize LLM judge service: {}", e);
            Ok(None)
        }
    }
}
