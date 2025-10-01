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

## Usage

Once configured, you can use the following capabilities through your AI assistant:

### Tools

#### `Run` (Local Command Execution)

Executes a command on the local system. This tool is primarily intended for troubleshooting SSH connectivity issues when remote commands fail.

**Parameters:**
- `command` (required): The command to execute locally
- `args` (optional): Array of arguments to pass to the command

**Example:**
```json
{
  "command": "ssh",
  "args": ["-v", "user@remote-host", "echo", "test"]
}
```

#### `SSH` (Remote Command Execution)

Executes a command on a remote POSIX compatible system (Linux, BSD, macOS) system via SSH. This tool does **not** permit commands to be run with sudo.

**Parameters:**
- `command` (required): The command to execute
- `args` (optional): Array of arguments to pass to the command
- `remote_host` (required): The hostname or IP address of the remote system
- `remote_user` (optional): The username to connect as (defaults to current user)

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

#### `SSH Sudo` (Remote Command Execution with Sudo)

Executes a command on a remote POSIX compatible system (Linux, BSD, macOS) system via SSH. This tool **permits** commands to be run with sudo for administrative tasks.

**Parameters:**
- `command` (required): The command to execute (can include sudo)
- `args` (optional): Array of arguments to pass to the command
- `remote_host` (required): The hostname or IP address of the remote system
- `remote_user` (optional): The username to connect as (defaults to current user)

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

### Resources

#### SSH File Access Resource Template

Access file contents on remote POSIX compatible system (Linux, BSD, macOS) systems using the SSH resource template.

**URI Format:** `ssh://{user}@{host}/{path}`

**Examples:**

- `ssh://admin@webserver.example.com/etc/nginx/nginx.conf` - Read nginx configuration as admin user
- `ssh://192.168.1.100/var/log/syslog` - Read system log using current username
- `ssh://user@myserver/home/user/.bashrc` - Read user's bash configuration
- `ssh://root@database-server/etc/mysql/my.cnf` - Read MySQL configuration as root

**Features:**
- **Automatic User Detection**: If no user is specified, uses your current username
- **URL Encoding Support**: Handles percent-encoded paths for special characters
- **Comprehensive Error Handling**: Clear error messages for connection and file access issues
- **Secure Authentication**: Uses your existing SSH keys and configuration

**Usage in MCP Clients:**
Most MCP clients will automatically discover and present this resource template. You can reference remote files directly using the SSH URI format, and the client will fetch the content transparently.

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

- **Timeout**: Check network connectivity and SSH daemon status
- **Authentication**: Verify SSH key permissions (600 for private key, 644 for public key)
- **Path Issues**: Use absolute paths for commands when possible

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
