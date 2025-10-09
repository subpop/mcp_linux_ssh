use anyhow::Error;
use expand_tilde::expand_tilde;
use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
};
use std::ops::Deref;
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

#[mcp_tool(
    name = "run_ssh_command",
    description = "Run a command on a remote POSIX compatible system (Linux, BSD, macOS) system and return the output. This tool does not permit commands to be run with sudo.",
    title = "Run SSH Command"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct RunSSHCommand {
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to run the command on.
    pub remote_host: String,
    /// The private key to use for authentication. Defaults to ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// The command to run. This must be a single command. Arguments must be passed in the args parameter.
    pub cmd: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a key-value pair separated by an equal sign (=). The options are passed to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

impl RunSSHCommand {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_ssh_command", cmd = ?self.cmd, args = ?self.args, timeout_seconds = ?self.timeout_seconds);
        let _enter = _span.enter();

        let remote_user = self.remote_user.clone().unwrap_or(whoami::username());
        let private_key = self
            .private_key
            .clone()
            .unwrap_or("~/.ssh/id_ed25519".to_string())
            .to_string();
        let timeout_seconds = self.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = self
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        if self.cmd.contains("sudo") || self.args.iter().any(|arg| arg.contains("sudo")) {
            // sudo is not permitted for this tool.
            return Err(CallToolError::from_message(
                "You may not run commands with sudo using this tool",
            ));
        }

        match exec_ssh(
            &remote_user,
            &self.remote_host,
            &private_key,
            &self.cmd,
            &self
                .args
                .iter()
                .map(|arg| arg.as_str())
                .collect::<Vec<&str>>(),
            timeout_seconds,
            options_vec.as_deref(),
        )
        .await
        {
            Ok(output) => {
                // The command executed successfully. This doesn't mean it
                // succeeded, so output is returned as a successful tool call.
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let status_code = output.status.code();

                Ok(
                    CallToolResult::text_content(vec![TextContent::from(stdout.clone())])
                        .with_structured_content(super::map_from_output(
                            stdout,
                            stderr,
                            status_code,
                        )),
                )
            }
            Err(err) => Err(CallToolError::from_message(format!(
                "Failed to execute remote SSH command: {}",
                err
            ))),
        }
    }
}

#[mcp_tool(
    name = "run_ssh_sudo_command",
    description = "Run a command on a remote POSIX compatible system (Linux, \
    BSD, macOS) system and return the output. This tool explicitly runs \
    commands with sudo.",
    title = "Run SSH Sudo Command"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct RunSSHSudoCommand {
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to run the command on.
    pub remote_host: String,
    /// The private key to use for authentication. Defaults to ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// The command to run. This must be a single command. Arguments must be passed in the args parameter.
    pub cmd: String,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a key-value pair separated by an equal sign (=). The options are passed to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

impl RunSSHSudoCommand {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span = tracing::span!(tracing::Level::TRACE, "run_ssh_sudo_command", cmd = ?self.cmd, args = ?self.args, timeout_seconds = ?self.timeout_seconds);
        let _enter = _span.enter();

        let remote_user = self.remote_user.clone().unwrap_or(whoami::username());
        let private_key = self
            .private_key
            .clone()
            .unwrap_or("~/.ssh/id_ed25519".to_string())
            .to_string();
        let timeout_seconds = self.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = self
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        match exec_ssh(
            &remote_user,
            &self.remote_host,
            &private_key,
            "sudo",
            std::iter::once(self.cmd.as_str())
                .chain(self.args.iter().map(|arg| arg.as_str()))
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
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let status_code = output.status.code();

                Ok(
                    CallToolResult::text_content(vec![TextContent::from(stdout.clone())])
                        .with_structured_content(super::map_from_output(
                            stdout,
                            stderr,
                            status_code,
                        )),
                )
            }
            Err(err) => Err(CallToolError::from_message(format!(
                "Failed to execute remote SSH command with sudo: {}",
                err
            ))),
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
) -> Result<std::process::Output, Error> {
    let _span = tracing::span!(tracing::Level::TRACE, "exec_ssh", user = %user, host = %host, private_key = %private_key, command = %command, args = ?args, timeout_seconds = %timeout_seconds);
    let _enter = _span.enter();

    let expanded_key = expand_tilde(private_key)
        .map_err(|e| Error::msg(format!("Failed to expand private key path: {}", e)))?;
    let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
        Error::msg(format!(
            "Failed to convert private key to string: {}",
            private_key
        ))
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
                return Err(Error::msg(format!(
                    "SSH command timed out after {} seconds",
                    timeout_seconds
                )));
            }
        }
    };

    result.map_err(|e| Error::msg(format!("Failed to run SSH command: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_ssh_command_rejects_sudo() {
        let cmd = RunSSHCommand {
            remote_user: None,
            remote_host: "localhost".to_string(),
            private_key: None,
            cmd: "sudo".to_string(),
            args: vec!["ls".to_string()],
            timeout_seconds: Some(1),
            options: None,
        };

        let result = cmd.call_tool().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("sudo"));
    }

    #[test]
    fn test_run_ssh_sudo_command_struct_creation() {
        let cmd = RunSSHSudoCommand {
            remote_user: Some("testuser".to_string()),
            remote_host: "localhost".to_string(),
            private_key: Some("~/.ssh/test_key".to_string()),
            cmd: "apt".to_string(),
            args: vec!["update".to_string()],
            timeout_seconds: Some(60),
            options: None,
        };

        assert_eq!(cmd.remote_host, "localhost");
        assert_eq!(cmd.cmd, "apt");
    }
}
