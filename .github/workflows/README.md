# GitHub Actions Workflows

This directory contains GitHub Actions workflows for the mcp_linux_ssh project.

## Workflows

### CI (`ci.yml`)
Runs on every push and pull request to `main` and `develop` branches:
- **Formatting Check**: Ensures code follows Rust formatting standards
- **Clippy Linting**: Runs Rust's linter to catch common mistakes
- **Tests**: Executes the test suite
- **Build**: Compiles the project in release mode
- **Security Audit**: Checks for known security vulnerabilities in dependencies

### Release (`release.yml`)
Builds cross-platform binaries and creates GitHub releases:

#### Supported Platforms
- **Linux**:
  - `x86_64-unknown-linux-gnu` (standard glibc)
  - `x86_64-unknown-linux-musl` (static binary with musl)
  - `aarch64-unknown-linux-gnu` (ARM64)
- **Windows**:
  - `x86_64-pc-windows-msvc` (64-bit Windows)
- **macOS**:
  - `x86_64-apple-darwin` (Intel Mac)
  - `aarch64-apple-darwin` (Apple Silicon Mac)

#### Triggering a Release

1. **Automatic (Recommended)**: Create and push a git tag starting with `v`:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. **Manual**: Use the "Actions" tab in GitHub and manually trigger the "Release" workflow.

#### Release Assets
Each release includes:
- Compiled binaries for all supported platforms
- `checksums.txt` file with SHA256 checksums for verification
- Release notes (extracted from CHANGELOG.md if present)

#### Binary Naming Convention
- `mcp_linux_ssh-x86_64-linux` - Linux x86_64 (glibc)
- `mcp_linux_ssh-x86_64-linux-musl` - Linux x86_64 (musl)
- `mcp_linux_ssh-aarch64-linux` - Linux ARM64
- `mcp_linux_ssh-x86_64-windows.exe` - Windows 64-bit
- `mcp_linux_ssh-x86_64-macos` - macOS Intel
- `mcp_linux_ssh-aarch64-macos` - macOS Apple Silicon

## Usage Tips

### For Contributors
- The CI workflow will automatically run on your pull requests
- Make sure your code passes `cargo fmt --check` and `cargo clippy` before pushing
- Add tests for new functionality

### For Maintainers
- Create a `CHANGELOG.md` file to have automatic release notes
- Use semantic versioning for tags (e.g., `v1.0.0`, `v1.0.1`, `v2.0.0`)
- Pre-release versions can include `alpha`, `beta`, or `rc` (e.g., `v1.0.0-beta.1`)

### Binary Verification
After downloading a release binary, verify its integrity:
```bash
# Download the binary and checksums
curl -L -O https://github.com/subpop/mcp_linux_ssh/releases/download/v1.0.0/mcp_linux_ssh-x86_64-linux
curl -L -O https://github.com/subpop/mcp_linux_ssh/releases/download/v1.0.0/checksums.txt

# Verify the checksum
sha256sum -c checksums.txt --ignore-missing
```
