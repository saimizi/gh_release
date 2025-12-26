use thiserror::Error;

/// Custom error types for gh_release
#[derive(Error, Debug)]
pub enum GhrError {
    /// GitHub API returned an error response
    #[error("GitHub API error: {0}")]
    GitHubApi(String),

    /// Repository not found or access denied
    #[error("Repository '{owner}/{repo}' not found or access denied")]
    RepositoryNotFound { owner: String, repo: String },

    /// Release not found
    #[error("Release with tag '{tag}' not found")]
    ReleaseNotFound { tag: String },

    /// Git command failed
    #[error("Git command failed: {0}")]
    GitCommand(String),

    /// Git is not installed or not in PATH
    #[error("Git is not installed or not available in PATH")]
    GitNotInstalled,

    /// Network error (from reqwest)
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// IO error (file operations, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Invalid URL format
    #[error("Invalid URL format: {url}")]
    InvalidUrl { url: String },

    /// Git ref (branch/tag/commit) not found
    #[error("Ref '{ref_name}' not found in {owner}/{repo}")]
    RefNotFound {
        owner: String,
        repo: String,
        ref_name: String,
    },

    /// Search pattern parsing error
    #[error("Invalid search pattern: {0}")]
    InvalidSearchPattern(String),

    /// Missing required argument
    #[error("Missing required argument: {0}")]
    MissingArgument(String),

    /// No releases found in repository
    #[error("No releases found in repository")]
    NoReleases,

    /// Header value error
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    /// Regex parsing error
    #[error("Invalid regex pattern: {0}")]
    RegexError(#[from] regex::Error),

    /// Glob pattern parsing error
    #[error("Invalid glob pattern: {0}")]
    GlobError(#[from] globset::Error),

    /// JSON parsing/serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Generic error for simple string messages
    #[error("{0}")]
    Generic(String),
}

/// Custom result type for gh_release
pub type Result<T> = std::result::Result<T, GhrError>;

/// Helper trait to convert &str to GhrError
impl From<&str> for GhrError {
    fn from(s: &str) -> Self {
        GhrError::Generic(s.to_string())
    }
}

/// Helper trait to convert String to GhrError
impl From<String> for GhrError {
    fn from(s: String) -> Self {
        GhrError::Generic(s)
    }
}
