# gh_release

A fast, simple command-line tool for fetching and downloading GitHub release assets. Perfect for CI/CD pipelines, automation scripts, and development workflows.

## Features

- 🚀 **Fast and lightweight** - Single binary written in Rust
- 🔐 **Multiple authentication methods** - Token, token file, or `.netrc` support
- 🔒 **Private repository support** - Works seamlessly with private repos using proper authentication
- 🎯 **Asset filtering** - Download only the assets you need with comma-separated filters
- 📦 **Latest release shorthand** - Use `-d latest` to always get the newest release
- 📁 **Custom output directory** - Save downloads to any directory
- 📊 **Release information** - View detailed info about releases before downloading
- 🔄 **Cross-platform** - Works on Linux, macOS, and Windows

## Installation

### From Source

```bash
git clone https://github.com/yourusername/gh_release.git
cd gh_release
cargo build --release
sudo cp target/release/gh_release /usr/local/bin/
```

### Using Cargo

```bash
cargo install --path .
```

## Usage

```bash
gh_release [OPTIONS] --repo <REPO>
```

### Required Arguments

- `-r, --repo <REPO>` - GitHub repository in format "owner/repo"

### Optional Arguments

| Option | Short | Long | Description |
|--------|-------|------|-------------|
| Token | `-t` | `--token <TOKEN>` | GitHub API token for authentication |
| Token File | `-T` | `--token-file <PATH>` | Path to file containing GitHub token |
| Download | `-d` | `--download <VERSION>` | Download specific version (or "latest") |
| Filter | `-f` | `--filter <FILTERS>` | Filter assets by comma-separated patterns |
| Output Dir | `-o` | `--output-dir <PATH>` | Save downloads to specified directory |
| Info | `-i` | `--info <VERSIONS>` | Show info about specific versions (comma-separated) |
| Number | `-n` | `--num <NUM>` | Number of releases to list (default: 1) |
| Verbose | `-v` | `--verbose` | Increase verbosity (-v, -vv for more detail) |

## Examples

### List Latest Release

```bash
gh_release -r owner/repo
```

### List Multiple Releases

```bash
gh_release -r owner/repo -n 5
```

### Download Latest Release

```bash
# Download all assets from latest release
gh_release -r owner/repo -d latest

# Download to specific directory
gh_release -r owner/repo -d latest -o ./downloads
```

### Download Specific Version

```bash
gh_release -r owner/repo -d v1.2.3
```

### Download with Filtering

```bash
# Download only Linux amd64 packages
gh_release -r owner/repo -d latest -f "linux,amd64"

# Download only .deb files
gh_release -r owner/repo -d v1.0.0 -f ".deb"

# Multiple filters (downloads assets containing any of these)
gh_release -r owner/repo -d latest -f "linux,darwin,windows"
```

### View Release Information

```bash
# Show info about specific version
gh_release -r owner/repo -i v1.2.3

# Show info about multiple versions
gh_release -r owner/repo -i "v1.2.3,v1.2.2,v1.2.1"
```

### Private Repository Access

```bash
# Using token directly
gh_release -r owner/private-repo -d latest -t ghp_xxxxxxxxxxxx

# Using token from file
echo "ghp_xxxxxxxxxxxx" > ~/.github_token
gh_release -r owner/private-repo -d latest -T ~/.github_token

# Using .netrc (automatic)
# Add to ~/.netrc:
# machine github.com
# login your-username
# password ghp_xxxxxxxxxxxx
gh_release -r owner/private-repo -d latest
```

### CI/CD Pipeline Examples

#### GitHub Actions

```yaml
- name: Download release asset
  run: |
    gh_release -r owner/repo -d latest -f "linux,amd64" -o ./bin
```

#### GitLab CI

```yaml
download_release:
  script:
    - gh_release -r owner/repo -d v1.0.0 -t $GITHUB_TOKEN -o ./artifacts
```

#### Jenkins

```groovy
sh 'gh_release -r owner/repo -d latest -T /var/jenkins/.github_token'
```

## Authentication

gh_release supports three authentication methods (in priority order):

### 1. Direct Token (Highest Priority)

```bash
gh_release -r owner/repo -t ghp_xxxxxxxxxxxx -d latest
```

### 2. Token File

```bash
gh_release -r owner/repo -T ~/.github_token -d latest
```

The token file should contain only the token string, with optional whitespace.

### 3. .netrc File (Automatic)

Create or edit `~/.netrc`:

```
machine github.com
login your-username
password ghp_xxxxxxxxxxxx
```

Then run without explicit authentication:

```bash
gh_release -r owner/repo -d latest
```

### 4. Unauthenticated (Fallback)

For public repositories, you can run without authentication:

```bash
gh_release -r owner/public-repo -d latest
```

**Note:** Unauthenticated requests have lower rate limits (60 requests/hour).

## Creating a GitHub Token

1. Go to GitHub Settings → Developer settings → Personal access tokens
2. Click "Generate new token (classic)"
3. Give it a descriptive name (e.g., "gh_release CLI")
4. Select scopes:
   - `repo` (for private repositories)
   - `public_repo` (for public repositories only)
5. Click "Generate token"
6. Copy the token immediately (it won't be shown again)

## Verbosity Levels

Control output detail with the `-v` flag:

```bash
# Normal (INFO level)
gh_release -r owner/repo -d latest

# Debug output
gh_release -r owner/repo -d latest -v

# Trace output (very detailed)
gh_release -r owner/repo -d latest -vv
```

## Common Use Cases

### Deploy Latest Release to Server

```bash
#!/bin/bash
gh_release -r mycompany/app -d latest -f "linux,amd64" -o /tmp
sudo dpkg -i /tmp/app_*_amd64.deb
```

### Download All Platforms

```bash
#!/bin/bash
for platform in linux darwin windows; do
  gh_release -r owner/repo -d v1.0.0 -f "$platform" -o "./dist/$platform"
done
```

### Check for New Releases

```bash
#!/bin/bash
current_version="v1.2.3"
latest=$(gh_release -r owner/repo -n 1 2>&1 | grep "Tag:" | awk '{print $2}')

if [ "$latest" != "$current_version" ]; then
  echo "New version available: $latest"
  gh_release -r owner/repo -d latest -o ./updates
fi
```

## Error Handling

gh_release provides clear error messages:

```bash
# Release not found
$ gh_release -r owner/repo -d v99.99.99
Error: Release with tag 'v99.99.99' not found

# Repository not found or access denied
$ gh_release -r owner/nonexistent
Error: GitHub API request failed with status: 404

# Network error
$ gh_release -r owner/repo -d latest
Error: Failed to send request: connection timeout
```

## Building from Source

### Prerequisites

- Rust 1.70 or later
- Cargo

### Build

```bash
git clone https://github.com/yourusername/gh_release.git
cd gh_release
cargo build --release
```

The binary will be at `target/release/gh_release`.

### Run Tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

1. Fork the repository
2. Clone your fork: `git clone https://github.com/yourname/gh_release.git`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Make your changes and add tests
5. Run tests: `cargo test`
6. Run formatter: `cargo fmt`
7. Run linter: `cargo clippy`
8. Commit your changes: `git commit -am 'Add new feature'`
9. Push to the branch: `git push origin feature/my-feature`
10. Submit a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

Built with:
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime
- [serde](https://github.com/serde-rs/serde) - Serialization framework

## Support

If you encounter any issues or have questions:
- Open an issue on [GitHub Issues](https://github.com/yourusername/gh_release/issues)
- Check existing issues for solutions

## Changelog

### v0.2.0 (2025-12-18)
- ✨ Added `--output-dir` option to specify download directory
- ✨ Added support for `-d latest` to download the most recent release
- 🐛 Fixed private repository asset downloads with token authentication
- 🧹 Removed unused dependencies (toml, regex, jaq)
- 📚 Added comprehensive README with examples

### v0.1.0 (Initial Release)
- ✨ List releases from GitHub repositories
- ✨ Download specific release assets
- ✨ Filter assets by name patterns
- ✨ Multiple authentication methods (token, token file, .netrc)
- ✨ Display release information
- ✨ Configurable logging levels
