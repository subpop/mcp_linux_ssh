# MCP POSIX compatible system (Linux, BSD, macOS) SSH Server

An MCP (Model Context Protocol) server that enables AI assistants to run commands and access files on remote POSIX compatible system (Linux, BSD, macOS) systems via SSH. This server provides a secure way for AI models to perform system administration tasks, troubleshoot issues, execute commands, and read configuration files on remote POSIX compatible system (Linux, BSD, macOS) machines.

## Example Use Cases

This MCP server enables LLMs to perform system administration tasks across remote infrastructure.

### Issue Discovery
- **Web performance** → Check system resources (`top`, `free`, `df`), examine web server logs, analyze network connections
- **Authentication failures** → Investigate authentication logs (`/var/log/auth.log`), check service status (`systemctl status sshd`), verify user accounts
- **Database timeouts** → Examine database logs, check connection pools, analyze slow query logs, monitor system resources

### Troubleshooting
- **Comparative analysis**: Check for differences between different web servers, databases, and load balancers
- **Configuration analysis**: Read and analyze config files (`nginx.conf`, `my.cnf`, etc.)
- **Dependency tracking**: Check systemd units, network connections, and process relationships

### Operations
- **Deployment verification**: Verify services are running, configurations are correct, and applications are responding
- **Incident response**: Gather diagnostic information from multiple systems
- **Documentation generation**: Document system configurations, installed packages, and service dependencies

## Features

- **Tools**:
  - Local command execution for SSH troubleshooting
  - Remote SSH command execution (standard user permissions)
  - Remote SSH command execution with sudo support
  - File copying with rsync (preserves attributes, creates backups)
  - Patch application over SSH (apply diffs to remote files)
- **Configurable timeouts**: Per-command timeout settings to prevent blocking
- **Public key discovery**: List available public keys from `~/.ssh` directory
- **Authentication**: Uses existing SSH configuration and keys
- **SSH configuration**: Relies on existing SSH config file (`~/.ssh/config`) for user and key specification
- **Security**: Leverages SSH's built-in security features
- **System administrator persona**: Built-in instructions for system administration tasks
- **Error handling**: Error messages for connection and execution issues

## Prerequisites

- Rust toolchain (1.70 or later)
- SSH client installed on your system
- SSH access configured to your target Linux systems
- SSH keys set up for passwordless authentication (recommended)

## Building

1. Clone this repository:
```bash
git clone https://github.com/subpop/mcp_linux_ssh
cd mcp_linux_ssh
```

2. Build the project:
```bash
cargo build --release
```

The compiled binary will be available at `target/release/mcp_linux_ssh`.

## Setup Instructions

### Claude Desktop

1. Build the project as described above
2. Edit your Claude Desktop configuration file:
   - **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

3. Add the MCP server configuration:
```json
{
  "mcpServers": {
    "linux-ssh": {
      "command": "/path/to/your/mcp_linux_ssh/target/release/mcp_linux_ssh",
      "args": [],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

4. Restart Claude Desktop

### Gemini (via MCP)

Gemini doesn't have native MCP support, but you can use it through compatible MCP clients or adapters. Follow the general MCP client setup pattern:

1. Use an MCP-compatible client that supports Gemini
2. Configure the server path: `/path/to/your/mcp_linux_ssh/target/release/mcp_linux_ssh`
3. Set environment variables as needed

### VSCode (via MCP Extension)

1. Install an MCP extension for VSCode (such as the official MCP extension)
2. Open VSCode settings (Cmd/Ctrl + ,)
3. Search for "MCP" settings
4. Add a new MCP server configuration:
   - **Name**: `linux-ssh`
   - **Command**: `/path/to/your/mcp_linux_ssh/target/release/mcp_linux_ssh`
   - **Args**: `[]`
   - **Environment**: `{"RUST_LOG": "info"}`

### Cursor

1. Build the project as described above
2. Open Cursor settings
3. Navigate to the MCP servers section
4. Add a new server configuration:
```json
{
  "name": "linux-ssh",
  "command": "/path/to/your/mcp_linux_ssh/target/release/mcp_linux_ssh",
  "args": [],
  "env": {
    "RUST_LOG": "info"
  }
}
```

### Goose

1. Build the project as described above
2. Create or edit your Goose configuration file (`~/.config/goose/config.yaml` or similar)
3. Add the MCP server configuration:
```yaml
mcp_servers:
  linux-ssh:
    command: /path/to/your/mcp_linux_ssh/target/release/mcp_linux_ssh
    args: []
    env:
      RUST_LOG: info
```

## SSH Configuration

Before using this MCP server, ensure your SSH is properly configured:

### 1. SSH Key Setup

Generate an SSH key pair if you don't have one:
```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```

Copy your public key to the remote server:
```bash
ssh-copy-id user@remote-host
```

### 2. SSH Config File

Create or edit `~/.ssh/config` to configure connections. This is the recommended way to specify users, keys, and other SSH settings:

```
Host myserver
    HostName 192.168.1.100
    User myuser
    Port 22
    IdentityFile ~/.ssh/id_ed25519

Host production-server
    HostName prod.example.com
    User deploy
    IdentityFile ~/.ssh/production_key

Host staging-server
    HostName staging.example.com
    User deploy
    IdentityFile ~/.ssh/staging_key
```

### 3. Test SSH Connection

Verify you can connect without a password:
```bash
ssh myserver whoami
```

## Usage

Once configured, you can use the following capabilities through your AI assistant:

### Tools

#### `Run` (Local Command Execution)

Executes a command on the local system. This tool is primarily intended for troubleshooting SSH connectivity issues when remote commands fail.

**Parameters:**
- `command` (required): The command to execute locally
- `args` (optional): Array of arguments to pass to the command
- `timeout_seconds` (optional): Timeout in seconds for command execution (default: 30, set to 0 to disable)

**Examples:**
```json
{
  "command": "ssh",
  "args": ["-v", "user@remote-host", "echo", "test"]
}
```

```json
{
  "command": "ping",
  "args": ["-c", "5", "google.com"],
  "timeout_seconds": 10
}
```

#### `SSH` (Remote Command Execution)

Executes a command on a remote POSIX compatible system (Linux, BSD, macOS) system via SSH. This tool does **not** permit commands to be run with sudo.

**Parameters:**
- `command` (required): The command to execute
- `args` (optional): Array of arguments to pass to the command
- `remote_host` (required): The hostname, IP address, or SSH config alias of the remote system
- `timeout_seconds` (optional): Timeout in seconds for command execution (default: 30, set to 0 to disable)
- `options` (optional): Additional SSH options to pass via `-o` flag (array of "key=value" strings)

**Examples:**

```json
{
  "command": "ls",
  "args": ["-la", "/home"],
  "remote_host": "myserver"
}
```

```json
{
  "command": "systemctl",
  "args": ["status", "nginx"],
  "remote_host": "webserver.example.com"
}
```

#### `SSH Sudo` (Remote Command Execution with Sudo)

Executes a command on a remote POSIX compatible system (Linux, BSD, macOS) system via SSH. This tool **permits** commands to be run with sudo for administrative tasks.

**Parameters:**
- `command` (required): The command to execute (can include sudo)
- `args` (optional): Array of arguments to pass to the command
- `remote_host` (required): The hostname, IP address, or SSH config alias of the remote system
- `timeout_seconds` (optional): Timeout in seconds for command execution (default: 30, set to 0 to disable)
- `options` (optional): Additional SSH options to pass via `-o` flag (array of "key=value" strings)

**Examples:**

```json
{
  "command": "sudo",
  "args": ["systemctl", "restart", "nginx"],
  "remote_host": "webserver.example.com"
}
```

```json
{
  "command": "sudo",
  "args": ["tail", "-f", "/var/log/syslog"],
  "remote_host": "logserver"
}
```

#### `Copy_File` (File Transfer with Rsync)

Copies a file from the local machine to a remote system using rsync. Preserves file attributes (permissions, timestamps, ownership) and creates backups of existing files on the remote system.

**Parameters:**
- `source` (required): The path to the source file on the local machine
- `destination` (required): The destination path on the remote machine
- `remote_host` (required): The hostname, IP address, or SSH config alias of the remote system
- `timeout_seconds` (optional): Timeout in seconds for the copy operation (default: 30, set to 0 to disable)

**Features:**
- **Archive mode**: Preserves permissions, timestamps, ownership, and other file attributes
- **Automatic backups**: If a file exists at the destination, a backup is created with a `~` suffix
- **Secure transfer**: Uses SSH for encrypted file transfer

**Examples:**

```json
{
  "source": "/home/user/config.yaml",
  "destination": "/etc/myapp/config.yaml",
  "remote_host": "webserver.example.com"
}
```

```json
{
  "source": "./build/app.jar",
  "destination": "/opt/myapp/app.jar",
  "remote_host": "production-server",
  "timeout_seconds": 60
}
```

**Backup Behavior:**
When copying to an existing file, rsync creates a backup with the original filename plus a `~` suffix:
- Original file: `/etc/myapp/config.yaml`
- Backup file: `/etc/myapp/config.yaml~`
- New file: `/etc/myapp/config.yaml` (updated)

#### `Patch_File` (Apply Patches to Remote Files)

Applies a patch/diff to a file on a remote system via SSH. The patch content is streamed through stdin over the SSH connection to the remote `patch` command.

**Parameters:**
- `patch` (required): The patch/diff content to apply (unified diff format recommended)
- `remote_file` (required): The path to the file on the remote machine to patch
- `remote_host` (required): The hostname, IP address, or SSH config alias of the remote system
- `timeout_seconds` (optional): Timeout in seconds for the patch operation (default: 30, set to 0 to disable)
- `options` (optional): Additional SSH options to pass via `-o` flag (array of "key=value" strings)

**Features:**
- **Stdin streaming**: Patch content is securely streamed via SSH stdin
- **Automatic strip detection**: The `patch` command automatically detects the appropriate `-p` strip level
- **Unified diff support**: Works best with unified diff format (`diff -u` or `git diff`)
- **Context preservation**: Maintains file context for accurate patching

**Examples:**

**Basic usage:**
```json
{
  "patch": "--- config.yaml\n+++ config.yaml\n@@ -1,3 +1,3 @@\n-port: 8080\n+port: 9090\n",
  "remote_file": "/etc/myapp/config.yaml",
  "remote_host": "webserver.example.com"
}
```

**Applying a Git diff:**
```json
{
  "patch": "diff --git a/app.py b/app.py\nindex 1234567..abcdefg 100644\n--- a/app.py\n+++ b/app.py\n@@ -10,7 +10,7 @@ def main():\n-    return 'Hello'\n+    return 'Hello, World!'\n",
  "remote_file": "/opt/myapp/app.py",
  "remote_host": "production-server"
}
```

**With custom SSH options:**
```json
{
  "patch": "--- nginx.conf\n+++ nginx.conf\n@@ -5,1 +5,1 @@\n-worker_processes 2;\n+worker_processes 4;\n",
  "remote_file": "/etc/nginx/nginx.conf",
  "remote_host": "192.168.1.100",
  "options": ["StrictHostKeyChecking=no", "UserKnownHostsFile=/dev/null"]
}
```

**Workflow:**
1. Generate a diff locally: `diff -u original.txt modified.txt > changes.patch`
2. Read the patch content
3. Use `Patch_File` to apply it to the remote file
4. Verify the changes with `SSH` tool (e.g., `cat /path/to/file`)

**Notes:**
- The `patch` command must be installed on the remote system
- Ensure the file to be patched exists on the remote system
- If the patch fails to apply cleanly, check the output for conflicts
- The remote file path should be absolute or relative to the remote user's home directory
- For large patches or binary files, consider using `Copy_File` instead

**Common Patch Formats:**
- **Unified diff** (recommended): `diff -u old.txt new.txt`
- **Git diff**: `git diff file.txt`
- **Context diff**: `diff -c old.txt new.txt`

## Timeout Configuration

All commands support configurable timeouts to prevent indefinite blocking.

- **Default**: 30 seconds
- **Disable**: Set `timeout_seconds` to `0`
- **Custom**: Set any positive integer (seconds)

### Examples

```json
// Quick command
{
  "command": "whoami",
  "remote_host": "server1",
  "timeout_seconds": 5
}

// Long operation
{
  "command": "find",
  "args": ["/", "-name", "*.log"],
  "remote_host": "server1",
  "timeout_seconds": 300
}

// No timeout (monitoring)
{
  "command": "tail",
  "args": ["-f", "/var/log/syslog"],
  "remote_host": "server1",
  "timeout_seconds": 0
}
```

## LLM Judge (Optional)

The MCP server supports an optional LLM-based judge that evaluates tool calls before execution. This offers an additional layer of security by allowing another LLM to review commands and reject potentially dangerous operations.

### Configuration

The judge is configured entirely through environment variables. To enable the judge, set the required environment variables:

```bash
# Required: LLM provider service
export MCP_LINUX_SSH_JUDGE_SERVICE="openai"

# Required: Model name
export MCP_LINUX_SSH_JUDGE_MODEL="gpt-4o-mini"

# Required for OpenAI, Anthropic, Gemini: API key
export MCP_LINUX_SSH_JUDGE_API_KEY="sk-..."

# Optional: Custom base URL
export MCP_LINUX_SSH_JUDGE_BASE_URL="https://api.openai.com/v1"

# Optional: Timeout in seconds (default: 10)
export MCP_LINUX_SSH_JUDGE_TIMEOUT_SECONDS="10"

# Optional: Fail mode - "open" or "closed" (default: "open")
export MCP_LINUX_SSH_JUDGE_FAIL_MODE="open"

# Optional: Comma-separated list of tools to judge (default: all tools)
export MCP_LINUX_SSH_JUDGE_TOOLS="run_ssh_command,run_ssh_sudo_command,copy_file,patch_file,run_local_command"
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `MCP_LINUX_SSH_JUDGE_SERVICE` | Yes | - | LLM provider: `"openai"`, `"anthropic"`, `"gemini"`, or `"ollama"` |
| `MCP_LINUX_SSH_JUDGE_MODEL` | Yes | - | Model name (e.g., `"gpt-4o-mini"`, `"claude-3-5-sonnet-20241022"`) |
| `MCP_LINUX_SSH_JUDGE_API_KEY` | Yes* | - | API key for the provider (*not required for Ollama) |
| `MCP_LINUX_SSH_JUDGE_BASE_URL` | No | Provider default | Custom base URL for the API |
| `MCP_LINUX_SSH_JUDGE_TIMEOUT_SECONDS` | No | `10` | Timeout for LLM judge calls |
| `MCP_LINUX_SSH_JUDGE_FAIL_MODE` | No | `"open"` | Behavior when judge unavailable: `"open"` (allow) or `"closed"` (reject) |
| `MCP_LINUX_SSH_JUDGE_TOOLS` | No | All tools | Comma-separated list of tool names to judge |

### Supported Providers

- **OpenAI**: Set `MCP_LINUX_SSH_JUDGE_SERVICE="openai"` and provide `MCP_LINUX_SSH_JUDGE_API_KEY` and `MCP_LINUX_SSH_JUDGE_MODEL`
- **Anthropic**: Set `MCP_LINUX_SSH_JUDGE_SERVICE="anthropic"` and provide `MCP_LINUX_SSH_JUDGE_API_KEY` and `MCP_LINUX_SSH_JUDGE_MODEL`
- **Gemini**: Set `MCP_LINUX_SSH_JUDGE_SERVICE="gemini"` and provide `MCP_LINUX_SSH_JUDGE_API_KEY` and `MCP_LINUX_SSH_JUDGE_MODEL`
- **Ollama**: Set `MCP_LINUX_SSH_JUDGE_SERVICE="ollama"` and provide `MCP_LINUX_SSH_JUDGE_MODEL` (no API key needed)

### Fail Mode

The `MCP_LINUX_SSH_JUDGE_FAIL_MODE` setting controls behavior when the judge is unavailable:

- **`"open"`** (default): If the judge fails or times out, allow the tool call to proceed
- **`"closed"`**: If the judge fails or times out, reject the tool call

### Tool Selection

Only tools listed in `MCP_LINUX_SSH_JUDGE_TOOLS` will be evaluated. Available tool names:
- `"run_local_command"` - Local command execution
- `"run_ssh_command"` - Remote SSH command execution
- `"run_ssh_sudo_command"` - Remote SSH command with sudo
- `"copy_file"` - File transfer with rsync
- `"patch_file"` - Apply patches to remote files

If `MCP_LINUX_SSH_JUDGE_TOOLS` is not set, all tools are judged by default.

### Judge Response Format

The judge must return JSON in this format:

```json
{
  "allowed": false,
  "reason": "Command attempts to delete root filesystem"
}
```

If `allowed` is `false`, the tool call is rejected with the `reason` as the error message.

### Example Usage

**OpenAI Example:**
```bash
export MCP_LINUX_SSH_JUDGE_SERVICE="openai"
export MCP_LINUX_SSH_JUDGE_MODEL="gpt-4o-mini"
export MCP_LINUX_SSH_JUDGE_API_KEY="sk-..."
export MCP_LINUX_SSH_JUDGE_FAIL_MODE="open"
```

**Anthropic Example:**
```bash
export MCP_LINUX_SSH_JUDGE_SERVICE="anthropic"
export MCP_LINUX_SSH_JUDGE_MODEL="claude-3-5-sonnet-20241022"
export MCP_LINUX_SSH_JUDGE_API_KEY="sk-ant-..."
export MCP_LINUX_SSH_JUDGE_TIMEOUT_SECONDS="15"
```

**Ollama Example (Local):**
```bash
export MCP_LINUX_SSH_JUDGE_SERVICE="ollama"
export MCP_LINUX_SSH_JUDGE_MODEL="llama3.2"
export MCP_LINUX_SSH_JUDGE_BASE_URL="http://localhost:11434"
```

### Disabling the Judge

If `MCP_LINUX_SSH_JUDGE_SERVICE` is not set, the judge is disabled and all tool calls proceed normally.

## Troubleshooting

### Connection Issues

1. **Permission Denied**: Ensure SSH keys are properly set up and the user has access
2. **Host Key Verification Failed**: Add the host to your known_hosts file:
   ```bash
   ssh-keyscan -H remote-host >> ~/.ssh/known_hosts
   ```
3. **Command Not Found**: Ensure the command exists on the remote system and is in the PATH

### Common SSH Issues

- **Connection Timeout**: Check network connectivity and SSH daemon status
- **Authentication**: Verify SSH key permissions (600 for private key, 644 for public key)
- **Path Issues**: Use absolute paths for commands when possible

### SSH Configuration Issues

1. **User Not Specified**: Ensure your `~/.ssh/config` has the `User` directive for each host:
   ```
   Host myserver
       User myuser
       IdentityFile ~/.ssh/id_ed25519
   ```

2. **Wrong Key Path**: Verify the `IdentityFile` in your SSH config points to the correct key:
   ```bash
   # Check if key exists
   ls -la ~/.ssh/id_ed25519

   # Verify key permissions
   chmod 600 ~/.ssh/id_ed25519
   ```

3. **Key Not Added to Agent**: If using SSH agent, ensure your key is loaded:
   ```bash
   ssh-add ~/.ssh/id_ed25519
   ssh-add -l  # List loaded keys
   ```

4. **Multiple Keys**: Configure different keys for different hosts in `~/.ssh/config`:
   ```
   Host production
       HostName prod.example.com
       User deploy
       IdentityFile ~/.ssh/production_key

   Host staging
       HostName staging.example.com
       User deploy
       IdentityFile ~/.ssh/staging_key
   ```

5. **Key Format Issues**: Ensure your private key is in the correct format (OpenSSH format preferred)

## Development

To modify or extend this server:

1. Edit the source code in `src/lib.rs`
2. Add new tools by implementing functions with the `#[tool]` attribute
3. Rebuild with `cargo build --release`
4. Restart your MCP client to pick up changes

### Logging

The server automatically logs all tool calls to `~/.local/state/mcp_linux_ssh/tool_calls.jsonl` (following XDG Base Directory specification) for debugging and audit purposes. Logs are rotated daily.

## Contributing

Contributions are welcome! Please ensure:
- Code is properly formatted (`cargo fmt`)
- All tests pass (`cargo test`)
- Security best practices are followed

## License

See [LICENSE](LICENSE) file for details.
