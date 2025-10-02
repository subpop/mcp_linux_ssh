# MCP POSIX compatible system (Linux, BSD, macOS) SSH Server

An MCP (Model Context Protocol) server that enables AI assistants to run commands and access files on remote POSIX compatible system (Linux, BSD, macOS) systems via SSH. This server provides a secure way for AI models to perform system administration tasks, troubleshoot issues, execute commands, and read configuration files on remote POSIX compatible system (Linux, BSD, macOS) machines.

## Example Use Cases

This MCP server enables LLMs to act as intelligent system administrators, capable of discovering and troubleshooting complex issues across your infrastructure. Here are some powerful examples:

### 🔍 **Automated Issue Discovery**
- **"My web application is responding slowly"** → LLM checks system resources (`top`, `free`, `df`), examines web server logs, analyzes network connections, and identifies the bottleneck
- **"Users can't log in"** → LLM investigates authentication logs (`/var/log/auth.log`), checks service status (`systemctl status sshd`), verifies user accounts, and diagnoses the root cause
- **"Database queries are timing out"** → LLM examines database logs, checks connection pools, analyzes slow query logs, and monitors system resources to pinpoint performance issues

### 🛠️ **Intelligent Troubleshooting**
- **Multi-server correlation**: LLM can simultaneously check logs and metrics across web servers, databases, and load balancers to trace issues through your entire stack
- **Configuration analysis**: Automatically read and analyze config files (`nginx.conf`, `my.cnf`, etc.) to identify misconfigurations or optimization opportunities  
- **Dependency tracking**: Follow service dependencies by checking systemd units, network connections, and process relationships

### 🚀 **Operational Efficiency**
- **Deployment verification**: After deployments, automatically verify services are running, configurations are correct, and applications are responding properly
- **Incident response**: During outages, quickly gather diagnostic information from multiple systems to accelerate root cause analysis
- **Documentation generation**: Automatically document system configurations, installed packages, and service dependencies

## Features

- **Three Command Execution Tools**: 
  - Local command execution for SSH troubleshooting
  - Remote SSH command execution (standard user permissions)
  - Remote SSH command execution with sudo support
- **Configurable Timeouts**: Prevent commands from blocking indefinitely with per-command timeout settings
- **Remote File Access**: Read file contents from remote systems using SSH resource templates
- **Flexible Authentication**: Uses your existing SSH configuration and keys
- **User Specification**: Option to specify which user to run commands as
- **Secure**: Leverages SSH's built-in security features
- **Expert Instructions**: Comes with built-in system administrator persona
- **Comprehensive Error Handling**: Clear error messages for connection and execution issues

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

Create or edit `~/.ssh/config` to simplify connections:
```
Host myserver
    HostName 192.168.1.100
    User myuser
    Port 22
    IdentityFile ~/.ssh/id_ed25519
```

### 3. Test SSH Connection

Verify you can connect without a password:
```bash
ssh myserver whoami
```

### 4. Custom Identity Files

The MCP server supports specifying custom private key files through the `private_key` parameter. This is useful when:

- You have multiple SSH keys for different servers or environments
- You need to use a specific key that's not the default (`~/.ssh/id_ed25519`)
- You're managing multiple remote systems with different authentication requirements

**Key Features:**
- **Default Behavior**: Uses `~/.ssh/id_ed25519` if no `private_key` is specified
- **Tilde Expansion**: Supports `~` for home directory (e.g., `~/.ssh/my_key`)
- **Absolute Paths**: Supports full paths (e.g., `/home/user/.ssh/production_key`)
- **Relative Paths**: Supports relative paths from current working directory

**Examples:**
```json
// Using default key (~/.ssh/id_ed25519)
{
  "command": "ls",
  "remote_host": "server1"
}

// Using custom key with tilde expansion
{
  "command": "ls", 
  "remote_host": "server2",
  "private_key": "~/.ssh/production_key"
}

// Using absolute path
{
  "command": "ls",
  "remote_host": "server3", 
  "private_key": "/opt/keys/deployment_key"
}
```

**Security Notes:**
- Ensure private key files have correct permissions (600)
- Keep private keys secure and never share them
- Use different keys for different environments (dev/staging/production)

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
- `remote_host` (required): The hostname or IP address of the remote system
- `remote_user` (optional): The username to connect as (defaults to current user)
- `private_key` (optional): Path to the private key file for authentication (defaults to `~/.ssh/id_ed25519`)
- `timeout_seconds` (optional): Timeout in seconds for command execution (default: 30, set to 0 to disable)

**Examples:**

```json
{
  "command": "ls",
  "args": ["-la", "/home"],
  "remote_host": "myserver",
  "remote_user": "admin"
}
```

```json
{
  "command": "systemctl",
  "args": ["status", "nginx"],
  "remote_host": "webserver.example.com"
}
```

**Using a custom private key:**

```json
{
  "command": "ps",
  "args": ["aux"],
  "remote_host": "production-server",
  "remote_user": "deploy",
  "private_key": "~/.ssh/production_key"
}
```

#### `SSH Sudo` (Remote Command Execution with Sudo)

Executes a command on a remote POSIX compatible system (Linux, BSD, macOS) system via SSH. This tool **permits** commands to be run with sudo for administrative tasks.

**Parameters:**
- `command` (required): The command to execute (can include sudo)
- `args` (optional): Array of arguments to pass to the command
- `remote_host` (required): The hostname or IP address of the remote system
- `remote_user` (optional): The username to connect as (defaults to current user)
- `private_key` (optional): Path to the private key file for authentication (defaults to `~/.ssh/id_ed25519`)
- `timeout_seconds` (optional): Timeout in seconds for command execution (default: 30, set to 0 to disable)

**Examples:**

```json
{
  "command": "sudo",
  "args": ["systemctl", "restart", "nginx"],
  "remote_host": "webserver.example.com",
  "remote_user": "admin"
}
```

```json
{
  "command": "sudo",
  "args": ["tail", "-f", "/var/log/syslog"],
  "remote_host": "logserver",
  "remote_user": "sysadmin"
}
```

**Using a custom private key with sudo:**

```json
{
  "command": "sudo",
  "args": ["systemctl", "restart", "apache2"],
  "remote_host": "web01.example.com",
  "remote_user": "admin",
  "private_key": "/home/user/.ssh/admin_key"
}
```

**Features:**
- **Automatic User Detection**: If no user is specified, uses your current username
- **URL Encoding Support**: Handles percent-encoded paths for special characters
- **Comprehensive Error Handling**: Clear error messages for connection and file access issues
- **Secure Authentication**: Uses your existing SSH keys and configuration

**Usage in MCP Clients:**
Most MCP clients will automatically discover and present this resource template. You can reference remote files directly using the SSH URI format, and the client will fetch the content transparently.

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

## Security Considerations

- **SSH Key Security**: Use strong SSH keys and keep private keys secure
- **Limited Scope**: Only grant access to systems you trust the AI to manage
- **User Permissions**: The remote user should have appropriate but limited permissions
- **Monitoring**: Consider logging SSH sessions for audit purposes
- **Network Security**: Ensure SSH is properly configured (disable password auth, use non-standard ports, etc.)

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

### Private Key Issues

1. **Wrong Key Path**: Ensure the `private_key` parameter points to the correct file:
   ```bash
   # Check if key exists
   ls -la ~/.ssh/id_ed25519
   
   # Verify key permissions
   chmod 600 ~/.ssh/id_ed25519
   ```

2. **Key Not Added to Agent**: If using SSH agent, ensure your key is loaded:
   ```bash
   ssh-add ~/.ssh/id_ed25519
   ssh-add -l  # List loaded keys
   ```

3. **Multiple Keys**: When using multiple keys, specify the correct one:
   ```json
   {
     "command": "whoami",
     "remote_host": "server",
     "private_key": "~/.ssh/specific_key"
   }
   ```

4. **Key Format Issues**: Ensure your private key is in the correct format (OpenSSH format preferred)

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
