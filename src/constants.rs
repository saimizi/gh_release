/// GitHub API base URL
pub const GITHUB_API_BASE: &str = "https://api.github.com";

/// GitHub API version
pub const GITHUB_API_VERSION: &str = "2022-11-28";

/// User agent for API requests
pub const USER_AGENT: &str = concat!("ghr/", env!("CARGO_PKG_VERSION"));

/// Default concurrency for parallel downloads
pub const DEFAULT_CONCURRENCY: usize = 5;

/// Default number of releases to fetch
pub const DEFAULT_NUM_RELEASES: usize = 10;

/// API endpoints
pub mod endpoints {
    use super::GITHUB_API_BASE;

    /// Get releases for a repository
    pub fn releases(owner: &str, repo: &str) -> String {
        format!("{}/repos/{}/{}/releases", GITHUB_API_BASE, owner, repo)
    }

    /// Get a specific release by tag
    pub fn release_by_tag(owner: &str, repo: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/{}/releases/tags/{}",
            GITHUB_API_BASE, owner, repo, tag
        )
    }

    /// Get repository information
    pub fn repository(owner: &str, repo: &str) -> String {
        format!("{}/repos/{}/{}", GITHUB_API_BASE, owner, repo)
    }

    /// Get branch information
    pub fn branch(owner: &str, repo: &str, branch: &str) -> String {
        format!(
            "{}/repos/{}/{}/branches/{}",
            GITHUB_API_BASE, owner, repo, branch
        )
    }

    /// Get tag information
    pub fn tag(owner: &str, repo: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/{}/git/refs/tags/{}",
            GITHUB_API_BASE, owner, repo, tag
        )
    }

    /// Get commit information
    pub fn commit(owner: &str, repo: &str, sha: &str) -> String {
        format!(
            "{}/repos/{}/{}/commits/{}",
            GITHUB_API_BASE, owner, repo, sha
        )
    }

    /// Search repositories
    pub fn search_repositories(query: &str, num: usize) -> String {
        format!(
            "{}/search/repositories?q={}&sort=stars&order=desc&per_page={}",
            GITHUB_API_BASE,
            urlencoding::encode(query),
            num
        )
    }
}

/// HTTP headers
pub mod headers {
    /// Accept header for GitHub API v3
    pub const ACCEPT_API_V3: &str = "application/vnd.github.v3+json";

    /// Accept header for downloading assets
    pub const ACCEPT_OCTET_STREAM: &str = "application/octet-stream";
}

/// Retry configuration
pub mod retry {
    /// Maximum number of retry attempts
    pub const MAX_RETRIES: u32 = 3;

    /// Base delay in seconds for exponential backoff
    pub const BASE_DELAY_SECS: u64 = 2;
}
