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
    #[allow(dead_code)]
    pub fn releases(owner: &str, repo: &str) -> String {
        releases_with_base(GITHUB_API_BASE, owner, repo)
    }

    /// Get releases with custom base URL
    pub fn releases_with_base(base_url: &str, owner: &str, repo: &str) -> String {
        format!("{}/repos/{}/{}/releases", base_url, owner, repo)
    }

    /// Get a specific release by tag
    #[allow(dead_code)]
    pub fn release_by_tag(owner: &str, repo: &str, tag: &str) -> String {
        release_by_tag_with_base(GITHUB_API_BASE, owner, repo, tag)
    }

    /// Get a specific release by tag with custom base URL
    pub fn release_by_tag_with_base(base_url: &str, owner: &str, repo: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/{}/releases/tags/{}",
            base_url, owner, repo, tag
        )
    }

    /// Get repository information
    #[allow(dead_code)]
    pub fn repository(owner: &str, repo: &str) -> String {
        repository_with_base(GITHUB_API_BASE, owner, repo)
    }

    /// Get repository information with custom base URL
    pub fn repository_with_base(base_url: &str, owner: &str, repo: &str) -> String {
        format!("{}/repos/{}/{}", base_url, owner, repo)
    }

    /// Get branch information
    #[allow(dead_code)]
    pub fn branch(owner: &str, repo: &str, branch: &str) -> String {
        branch_with_base(GITHUB_API_BASE, owner, repo, branch)
    }

    /// Get branch information with custom base URL
    pub fn branch_with_base(base_url: &str, owner: &str, repo: &str, branch: &str) -> String {
        format!("{}/repos/{}/{}/branches/{}", base_url, owner, repo, branch)
    }

    /// Get tag information
    #[allow(dead_code)]
    pub fn tag(owner: &str, repo: &str, tag: &str) -> String {
        tag_with_base(GITHUB_API_BASE, owner, repo, tag)
    }

    /// Get tag information with custom base URL
    pub fn tag_with_base(base_url: &str, owner: &str, repo: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/{}/git/refs/tags/{}",
            base_url, owner, repo, tag
        )
    }

    /// Get commit information
    #[allow(dead_code)]
    pub fn commit(owner: &str, repo: &str, sha: &str) -> String {
        commit_with_base(GITHUB_API_BASE, owner, repo, sha)
    }

    /// Get commit information with custom base URL
    pub fn commit_with_base(base_url: &str, owner: &str, repo: &str, sha: &str) -> String {
        format!("{}/repos/{}/{}/commits/{}", base_url, owner, repo, sha)
    }

    /// Search repositories
    #[allow(dead_code)]
    pub fn search_repositories(query: &str, num: usize) -> String {
        search_repositories_with_base(GITHUB_API_BASE, query, num)
    }

    /// Search repositories with custom base URL
    pub fn search_repositories_with_base(base_url: &str, query: &str, num: usize) -> String {
        format!(
            "{}/search/repositories?q={}&sort=stars&order=desc&per_page={}",
            base_url,
            urlencoding::encode(query),
            num
        )
    }

    /// Get tags for a repository
    #[allow(dead_code)]
    pub fn tags(owner: &str, repo: &str, per_page: usize) -> String {
        tags_with_base(GITHUB_API_BASE, owner, repo, per_page)
    }

    /// Get tags for a repository with custom base URL
    pub fn tags_with_base(base_url: &str, owner: &str, repo: &str, per_page: usize) -> String {
        format!(
            "{}/repos/{}/{}/tags?per_page={}",
            base_url, owner, repo, per_page
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
