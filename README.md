# gh_release

A fast, simple command-line tool for fetching and downloading GitHub release assets. Perfect for CI/CD pipelines, automation scripts, and development workflows.

## Features

- 🚀 **Fast and lightweight** - Single binary written in Rust
- 🔐 **Multiple authentication methods** - Token, token file, or `.netrc` support
- 🔒 **Private repository support** - Works seamlessly with private repos using proper authentication
- 🎯 **Advanced asset filtering** - Glob patterns, regex, and exclusion filters
- 📦 **Latest release shorthand** - Use `-d latest` to always get the newest release
- 📁 **Custom output directory** - Save downloads to any directory
- 📊 **Release information** - View detailed info about releases before downloading
- 🔄 **Cross-platform** - Works on Linux, macOS, and Windows
- 🏢 **GitHub Enterprise support** - Custom API base URLs for enterprise instances
- 💾 **Response caching** - Optional caching to reduce API calls
- 🎭 **Dry-run mode** - Preview operations without executing them
- 📋 **JSON output** - Machine-readable output for scripting

## Installation

### From Source

```bash
git clone https://github.com/saimizi/gh_release.git
cd gh_release
cargo build --release
sudo cp target/release/ghr /usr/local/bin/
```

### Using Cargo

```bash
cargo install --path .
```

## Usage

```bash
ghr [OPTIONS] --repo <REPO>
```

### Required Arguments

- `-r, --repo <REPO>` - GitHub repository in format "owner/repo"

### Optional Arguments

| Option | Short | Long | Description |
|--------|-------|------|-------------|
| Token | `-t` | `--token <TOKEN>` | GitHub API token for authentication |
| Token File | `-T` | `--token-file <PATH>` | Path to file containing GitHub token |
| Clone | `-c` | `--clone <URL[:REF]>` | Clone repository with optional branch/tag/commit |
| Download | `-d` | `--download <VERSION>` | Download specific version (or "latest") |
| Filter | `-f` | `--filter <FILTERS>` | Filter assets by patterns (glob/regex/exclude) |
| Info | `-i` | `--info <VERSIONS>` | Show info about specific versions (comma-separated) |
| Search | `-s` | `--search <PATTERN>` | Search for repositories |
| Number | `-n` | `--num <NUM>` | Number of releases to list (default: 10) |
| Concurrency | `-j` | `--concurrency <NUM>` | Maximum number of concurrent downloads (default: 5) |
| Dry-run | | `--dry-run` | Preview operations without executing them |
| Format | | `--format <FORMAT>` | Output format: table (default) or json |
| API URL | | `--api-url <URL>` | GitHub API base URL (for GitHub Enterprise) |
| Cache | | `--cache` | Enable response caching (24 hour TTL) |
| Verbose | `-v` | `--verbose` | Increase verbosity (-v, -vv for more detail) |

### Positional Arguments

| Argument | Description |
|----------|-------------|
| `[DIRECTORY]` | Directory for downloads or clone destination |

## Examples

### List Latest Release

```bash
ghr -r owner/repo
```

### List Multiple Releases

```bash
ghr -r owner/repo -n 5
```

### Download Latest Release

```bash
# Download all assets from latest release to current directory
ghr -r owner/repo -d latest

# Download to specific directory
ghr -r owner/repo -d latest ./downloads
```

### Download Specific Version

```bash
# Download to current directory
ghr -r owner/repo -d v1.2.3

# Download to specific directory
ghr -r owner/repo -d v1.2.3 ./releases
```

### Download with Filtering

The filter system supports multiple pattern types:

#### Substring Matching (Simple)
```bash
# Download assets containing "linux" AND "amd64"
ghr -r owner/repo -d latest -f "linux,amd64"
```

#### Glob Patterns (Wildcards)
```bash
# Download only .deb files
ghr -r owner/repo -d latest -f "*.deb"

# Download .tar.gz files
ghr -r owner/repo -d latest -f "*.tar.gz"

# Download files starting with "app-"
ghr -r owner/repo -d latest -f "app-*"
```

#### Regex Patterns (Advanced)
```bash
# Download linux packages for amd64 architecture
ghr -r owner/repo -d latest -f "linux-.*-amd64"

# Download versioned releases
ghr -r owner/repo -d latest -f "app-v[0-9]+\\..*"
```

#### Exclusion Patterns
```bash
# Download everything except Windows binaries
ghr -r owner/repo -d latest -f "!windows"

# Download .deb files but not test packages
ghr -r owner/repo -d latest -f "*.deb,!test"
```

#### Combined Patterns
```bash
# Multiple filters work with AND logic
ghr -r owner/repo -d latest -f "*.deb,!test,linux"
# Downloads: .deb files, excluding test packages, containing "linux"
```

### Clone Repository

Clone a GitHub repository with optional branch, tag, or commit checkout:

```bash
# Clone repository to default directory (repository name)
ghr -c owner/repo

# Clone to specific directory
ghr -c owner/repo my-directory

# Clone specific branch
ghr -c owner/repo:main my-directory

# Clone specific tag
ghr -c owner/repo:v1.0.0 my-directory

# Clone specific commit
ghr -c owner/repo:abc1234 my-directory

# Clone with HTTPS URL
ghr -c https://github.com/owner/repo

# Clone private repository (requires authentication)
ghr -t YOUR_TOKEN -c owner/private-repo my-directory
```

**Supported URL formats:**
- `owner/repo` (short format)
- `https://github.com/owner/repo`
- `https://github.com/owner/repo.git`
- `git@github.com:owner/repo.git`

**Optional ref specification:**
Append `:ref` to specify branch, tag, or commit SHA to checkout after cloning (e.g., `owner/repo:main`).

**Prerequisites:**
- Git must be installed and available in PATH

### View Release Information

```bash
# Show info about specific version
ghr -r owner/repo -i v1.2.3

# Show info about multiple versions
ghr -r owner/repo -i "v1.2.3,v1.2.2,v1.2.1"
```

### Search Repositories

Search for GitHub repositories using flexible patterns:

```bash
# List all repositories owned by a user
ghr -s "torvalds/"

# Search user's repositories containing keyword
ghr -s "rust-lang/compiler"

# Search top N repositories globally
ghr -s "/kubernetes" -n 10
```

#### Search Pattern Formats

| Pattern | Description | Example |
|---------|-------------|---------|
| `username/` | List all repos owned by user/org | `ghr -s "microsoft/"` |
| `username/keyword` | Search user's repos with keyword | `ghr -s "google/tensorflow"` |
| `/keyword` | Search top repos globally | `ghr -s "/docker"` |

**Note**: Use `-n` flag to control number of results (default: 10)

### Dry-Run Mode

Preview what will be downloaded or cloned without executing:

```bash
# Preview download without downloading
ghr -r owner/repo -d latest --dry-run

# Output shows:
# - List of assets that would be downloaded
# - Size of each asset
# - Total download size
# - Destination directory

# Preview clone operation
ghr -c owner/repo:main --dry-run

# Output shows:
# - Repository to be cloned
# - Branch/tag/commit to checkout
# - Target directory
```

### JSON Output

Get machine-readable output for scripting and automation:

```bash
# List releases in JSON format
ghr -r owner/repo --format json

# Search repositories in JSON format
ghr -s "rust-lang/" --format json -n 5

# Parse with jq
ghr -r owner/repo --format json | jq '.[0].tag_name'
ghr -r owner/repo --format json | jq -r '.[] | .assets[].name'
```

### Response Caching

Enable caching to reduce API calls and improve performance:

```bash
# Enable caching (24 hour TTL)
ghr -r owner/repo --cache

# Subsequent calls use cached data
ghr -r owner/repo --cache  # Fast! Uses cache

# Works with all API operations
ghr -s "microsoft/" --cache -n 10
ghr -r owner/repo -i v1.0.0 --cache
```

**Benefits:**
- Reduces API rate limit usage
- Faster response times for repeated queries
- Cache stored in `~/.cache/ghr/` (or platform equivalent)
- Automatic expiration after 24 hours

### GitHub Enterprise Support

Use with GitHub Enterprise instances:

```bash
# Specify custom API URL
ghr --api-url https://github.enterprise.com/api -r owner/repo -d latest

# Works with all operations
ghr --api-url https://ghe.company.com/api -s "team/" -n 10
ghr --api-url https://ghe.company.com/api -c owner/repo
```

### Private Repository Access

```bash
# Using token directly
ghr -r owner/private-repo -d latest -t ghp_xxxxxxxxxxxx

# Using token from file
echo "ghp_xxxxxxxxxxxx" > ~/.github_token
ghr -r owner/private-repo -d latest -T ~/.github_token

# Using .netrc (automatic)
# Add to ~/.netrc:
# machine github.com
# login your-username
# password ghp_xxxxxxxxxxxx
ghr -r owner/private-repo -d latest
```

### CI/CD Pipeline Examples

#### GitHub Actions

```yaml
- name: Download release asset
  run: |
    # Use advanced filtering and caching
    ghr -r owner/repo -d latest -f "*.deb,!test" --cache ./bin

- name: Check for new releases
  run: |
    # Use JSON output for scripting
    LATEST=$(ghr -r owner/repo --format json | jq -r '.[0].tag_name')
    echo "Latest version: $LATEST"
```

#### GitLab CI

```yaml
download_release:
  script:
    # Use dry-run first, then download
    - ghr -r owner/repo -d v1.0.0 -t $GITHUB_TOKEN --dry-run
    - ghr -r owner/repo -d v1.0.0 -t $GITHUB_TOKEN -f "linux-.*-amd64" ./artifacts
```

#### Jenkins

```groovy
// Use GitHub Enterprise and caching
sh 'ghr --api-url https://ghe.company.com/api -r owner/repo -d latest --cache -T /var/jenkins/.github_token ./bin'
```

## Authentication

ghr supports three authentication methods (in priority order):

### 1. Direct Token (Highest Priority)

```bash
ghr -r owner/repo -t ghp_xxxxxxxxxxxx -d latest
```

### 2. Token File

```bash
ghr -r owner/repo -T ~/.github_token -d latest
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
ghr -r owner/repo -d latest
```

### 4. Unauthenticated (Fallback)

For public repositories, you can run without authentication:

```bash
ghr -r owner/public-repo -d latest
```

**Note:** Unauthenticated requests have lower rate limits (60 requests/hour).

## Creating a GitHub Token

1. Go to GitHub Settings → Developer settings → Personal access tokens
2. Click "Generate new token (classic)"
3. Give it a descriptive name (e.g., "ghr CLI")
4. Select scopes:
   - `repo` (for private repositories)
   - `public_repo` (for public repositories only)
5. Click "Generate token"
6. Copy the token immediately (it won't be shown again)

## Verbosity Levels

Control output detail with the `-v` flag:

```bash
# Normal (INFO level)
ghr -r owner/repo -d latest

# Debug output
ghr -r owner/repo -d latest -v

# Trace output (very detailed)
ghr -r owner/repo -d latest -vv
```

## Common Use Cases

### Deploy Latest Release to Server

```bash
#!/bin/bash
# Preview first with dry-run
ghr -r mycompany/app -d latest -f "*.deb,!test" --dry-run

# Then download with advanced filtering
ghr -r mycompany/app -d latest -f "*.deb,!test,linux" /tmp
sudo dpkg -i /tmp/app_*_amd64.deb
```

### Download All Platforms

```bash
#!/bin/bash
for platform in linux darwin windows; do
  ghr -r owner/repo -d v1.0.0 -f "$platform" "./dist/$platform"
done
```

### Check for New Releases

```bash
#!/bin/bash
current_version="v1.2.3"

# Use JSON output with caching for efficiency
latest=$(ghr -r owner/repo --format json --cache | jq -r '.[0].tag_name')

if [ "$latest" != "$current_version" ]; then
  echo "New version available: $latest"
  # Preview before downloading
  ghr -r owner/repo -d latest --dry-run
  # Then download
  ghr -r owner/repo -d latest -f "linux-.*-amd64" ./updates
fi
```

## Error Handling

ghr provides clear error messages:

```bash
# Release not found
$ ghr -r owner/repo -d v99.99.99
Error: Release with tag 'v99.99.99' not found

# Repository not found or access denied
$ ghr -r owner/nonexistent
Error: GitHub API request failed with status: 404

# Network error
$ ghr -r owner/repo -d latest
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

The binary will be at `target/release/ghr`.

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
- [regex](https://github.com/rust-lang/regex) - Regular expressions
- [globset](https://github.com/BurntSushi/ripgrep/tree/master/crates/globset) - Glob pattern matching
- [thiserror](https://github.com/dtolnay/thiserror) - Error handling

## Support

If you encounter any issues or have questions:
- Open an issue on [GitHub Issues](https://github.com/yourusername/gh_release/issues)
- Check existing issues for solutions

