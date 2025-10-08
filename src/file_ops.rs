use anyhow::Result;
use expand_tilde::expand_tilde;
use rmcp::{ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::ops::Deref;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

use crate::ssh::SshConnectionParams;

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

impl super::Handler {
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
