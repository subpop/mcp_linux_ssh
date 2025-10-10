use expand_tilde::expand_tilde;
use rust_mcp_sdk::{
    macros::{JsonSchema, mcp_tool},
    schema::{CallToolResult, TextContent, schema_utils::CallToolError},
};
use std::ops::Deref;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::{Duration, timeout},
};

#[mcp_tool(
    name = "patch_file",
    description = "Apply a patch or diff to a file on the remote machine using the patch command. \
    The patch content is streamed via stdin over SSH. By default, patch will attempt to \
    automatically detect the correct strip level (-p). Use unified diff format for best results.",
    title = "Patch File"
)]
#[derive(Debug, ::serde::Serialize, ::serde::Deserialize, JsonSchema)]
pub struct PatchFile {
    /// The patch/diff content to apply.
    pub patch: String,
    /// The path to the file on the remote machine to patch.
    pub remote_file: String,
    /// The user to run the command as. Defaults to the current username.
    pub remote_user: Option<String>,
    /// The host to run the command on.
    pub remote_host: String,
    /// The private key to use for authentication. Defaults to ~/.ssh/id_ed25519.
    pub private_key: Option<String>,
    /// Timeout in seconds for the command execution. Defaults to 30 seconds. Set to 0 to disable timeout.
    pub timeout_seconds: Option<u64>,
    /// Additional options to pass to the ssh command. Each option should be a key-value pair separated by an equal sign (=). The options are passed to the ssh command using the -o flag.
    pub options: Option<Vec<String>>,
}

impl PatchFile {
    #[tracing::instrument(skip(self))]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let _span =
            tracing::span!(tracing::Level::TRACE, "patch_file", remote_file = ?self.remote_file);
        let _enter = _span.enter();

        let remote_user = self.remote_user.clone().unwrap_or(whoami::username());
        let private_key = self
            .private_key
            .clone()
            .unwrap_or("~/.ssh/id_ed25519".to_string());
        let timeout_seconds = self.timeout_seconds.unwrap_or(30);
        let options_vec: Option<Vec<&str>> = self
            .options
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());

        // Expand the private key path
        let expanded_key = expand_tilde(&private_key).map_err(|e| {
            CallToolError::from_message(format!("Failed to expand private key path: {}", e))
        })?;
        let private_key_path = expanded_key.deref().as_os_str().to_str().ok_or_else(|| {
            CallToolError::from_message(format!(
                "Failed to convert private key to string: {}",
                private_key
            ))
        })?;

        // Build SSH command that will run patch on the remote side
        // The patch command reads from stdin and applies to the specified file
        let mut cmd = Command::new("ssh");
        cmd.arg(&self.remote_host)
            .args(["-l", &remote_user])
            .args(["-i", private_key_path])
            .args(
                options_vec
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|opt| ["-o", opt]),
            )
            .arg("patch")
            .arg(&self.remote_file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let command_future = async {
            let mut child = cmd.spawn().map_err(|e| {
                CallToolError::from_message(format!("Failed to spawn SSH command: {}", e))
            })?;

            // Write the patch content to stdin
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(self.patch.as_bytes()).await.map_err(|e| {
                    CallToolError::from_message(format!("Failed to write patch to stdin: {}", e))
                })?;
                // Close stdin to signal EOF
                drop(stdin);
            }

            // Wait for the command to complete
            child.wait_with_output().await.map_err(|e| {
                CallToolError::from_message(format!("Failed to wait for SSH command: {}", e))
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
                    return Err(CallToolError::from_message(format!(
                        "Patch command timed out after {} seconds",
                        timeout_seconds
                    )));
                }
            }
        };

        match result {
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
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_file_struct_creation() {
        let patch_cmd = PatchFile {
            patch: "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new".to_string(),
            remote_file: "/home/user/file.txt".to_string(),
            remote_user: Some("testuser".to_string()),
            remote_host: "localhost".to_string(),
            private_key: Some("~/.ssh/test_key".to_string()),
            timeout_seconds: Some(60),
            options: Some(vec!["StrictHostKeyChecking=no".to_string()]),
        };

        assert_eq!(patch_cmd.remote_file, "/home/user/file.txt");
        assert_eq!(patch_cmd.remote_host, "localhost");
        assert!(patch_cmd.patch.contains("old"));
        assert!(patch_cmd.patch.contains("new"));
    }

    #[test]
    fn test_patch_file_defaults() {
        let patch_cmd = PatchFile {
            patch: "diff content".to_string(),
            remote_file: "/path/to/file".to_string(),
            remote_user: None,
            remote_host: "example.com".to_string(),
            private_key: None,
            timeout_seconds: None,
            options: None,
        };

        assert!(patch_cmd.remote_user.is_none());
        assert!(patch_cmd.private_key.is_none());
        assert!(patch_cmd.timeout_seconds.is_none());
        assert!(patch_cmd.options.is_none());
    }
}
