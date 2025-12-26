use clap::{ArgAction, Parser};

/// CLI arguments
#[derive(Parser)]
#[command(
    name = "Github release fetcher",
    version,
    about = "A tool to retrieve and download github release package."
)]
pub struct Cli {
    /// GitHub Repository in the format "owner/repo" (required for release operations)
    #[arg(long, short = 'r')]
    pub repo: Option<String>,

    /// Token for GitHub API authentication
    #[arg(short = 't', long = "token")]
    pub token: Option<String>,

    /// File containing GitHub API token
    #[arg(short = 'T', long = "token-file")]
    pub token_file: Option<String>,

    /// Specific version to download (or "latest" for the most recent release)
    #[arg(short = 'd', long = "download")]
    pub download: Option<String>,

    /// String used to filter the name of assets to download, multiple filters can be separated by
    /// commas.
    #[arg(short = 'f', long = "filter")]
    pub filter: Option<String>,

    /// Search for repositories using pattern:
    /// - "username/keyword": Search repos owned by username containing keyword
    /// - "username/": List all repos owned by username
    /// - "/keyword": Search top N repos globally containing keyword
    #[arg(short = 's', long = "search")]
    pub search: Option<String>,

    /// Show information about a specific version, multiple versions can be separated by commas.
    #[arg(short = 'i', long = "info")]
    pub info: Option<String>,

    /// Number of packages to fetch
    #[arg(short = 'n', long = "num", default_value_t = 10)]
    pub num: usize,

    /// Maximum number of concurrent downloads
    #[arg(short = 'j', long = "concurrency", default_value_t = 5)]
    pub concurrency: usize,

    /// Clone a repository with optional ref (branch/tag/sha1)
    /// Format: <url>[:<ref>] where url can be:
    ///   - https://github.com/owner/repo
    ///   - git@github.com:owner/repo.git
    ///   - owner/repo (short format)
    #[arg(short = 'c', long = "clone", value_name = "URL[:REF]")]
    pub clone: Option<String>,

    /// Directory for operation (clone destination or download location)
    /// - For clone: defaults to repository name
    /// - For download: defaults to current directory
    #[arg(value_name = "DIRECTORY")]
    pub directory: Option<String>,

    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    pub verbose: u8,
}
