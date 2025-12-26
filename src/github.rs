use crate::models::{Release, Repository, RepositoryInfo, Result, SearchResponse};
use jlogger_tracing::jinfo;
use reqwest::Client;

/// Fetch release information from GitHub
pub async fn get_release_info(
    client: &Client,
    repo: &str,
    tag: Option<&str>,
) -> Result<Vec<Release>> {
    let url = if let Some(tag) = tag {
        format!(
            "https://api.github.com/repos/{}/releases/tags/{}",
            repo, tag
        )
    } else {
        format!("https://api.github.com/repos/{}/releases", repo)
    };

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    if tag.is_some() {
        // Single release
        let release: Release = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        Ok(vec![release])
    } else {
        // Multiple releases
        let releases: Vec<Release> = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        Ok(releases)
    }
}

/// Search pattern types
#[derive(Debug)]
pub enum SearchPattern {
    UserWithKeyword { username: String, keyword: String },
    UserAllRepos { username: String },
    GlobalKeyword { keyword: String },
}

/// Parse search pattern from string
pub fn parse_search_pattern(pattern: &str) -> Result<SearchPattern> {
    let pattern = pattern.trim();

    if pattern.is_empty() {
        return Err("Search pattern cannot be empty".to_string());
    }

    if let Some(slash_pos) = pattern.find('/') {
        let username = &pattern[..slash_pos];
        let keyword = &pattern[slash_pos + 1..];

        if username.is_empty() {
            // Pattern: "/keyword"
            if keyword.is_empty() {
                return Err("Keyword cannot be empty for global search".to_string());
            }
            Ok(SearchPattern::GlobalKeyword {
                keyword: keyword.to_string(),
            })
        } else if keyword.is_empty() {
            // Pattern: "username/"
            Ok(SearchPattern::UserAllRepos {
                username: username.to_string(),
            })
        } else {
            // Pattern: "username/keyword"
            Ok(SearchPattern::UserWithKeyword {
                username: username.to_string(),
                keyword: keyword.to_string(),
            })
        }
    } else {
        // No slash - treat as global keyword search
        Ok(SearchPattern::GlobalKeyword {
            keyword: pattern.to_string(),
        })
    }
}

/// Search for repositories
pub async fn search_repositories(
    client: &Client,
    pattern: &SearchPattern,
    num: usize,
) -> Result<Vec<Repository>> {
    let query = match pattern {
        SearchPattern::UserWithKeyword { username, keyword } => {
            format!("user:{} {} in:name,description", username, keyword)
        }
        SearchPattern::UserAllRepos { username } => {
            format!("user:{}", username)
        }
        SearchPattern::GlobalKeyword { keyword } => {
            format!("{} in:name,description", keyword)
        }
    };

    let url = format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page={}",
        urlencoding::encode(&query),
        num
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    let search_response: SearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(search_response.items)
}

/// Validate that a repository exists and is accessible
pub async fn validate_repository(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<RepositoryInfo> {
    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    jinfo!("Validating repository {}/{}...", owner, repo);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to GitHub API: {}", e))?;

    if response.status().is_success() {
        let repo_info: RepositoryInfo = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse repository response: {}", e))?;
        Ok(repo_info)
    } else if response.status() == reqwest::StatusCode::NOT_FOUND {
        Err(format!(
            "Repository '{}/{}' not found (or you don't have access)",
            owner, repo
        ))
    } else {
        Err(format!(
            "GitHub API request failed with status: {}",
            response.status()
        ))
    }
}

/// Validate that a ref (branch/tag/commit) exists in a repository
pub async fn validate_ref(
    client: &Client,
    owner: &str,
    repo: &str,
    ref_name: &str,
) -> Result<String> {
    jinfo!("Validating ref '{}'...", ref_name);

    // Try as branch first
    let branch_url = format!(
        "https://api.github.com/repos/{}/{}/branches/{}",
        owner, repo, ref_name
    );

    let response = client.get(&branch_url).send().await.map_err(|e| {
        format!(
            "Failed to connect to GitHub API while checking branch: {}",
            e
        )
    })?;

    if response.status().is_success() {
        return Ok("branch".to_string());
    }

    // Try as tag
    let tag_url = format!(
        "https://api.github.com/repos/{}/{}/git/refs/tags/{}",
        owner, repo, ref_name
    );

    let response = client
        .get(&tag_url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to GitHub API while checking tag: {}", e))?;

    if response.status().is_success() {
        return Ok("tag".to_string());
    }

    // Try as commit SHA
    let commit_url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}",
        owner, repo, ref_name
    );

    let response = client.get(&commit_url).send().await.map_err(|e| {
        format!(
            "Failed to connect to GitHub API while checking commit: {}",
            e
        )
    })?;

    if response.status().is_success() {
        return Ok("commit".to_string());
    }

    // Ref not found
    Err(format!(
        "Branch/tag/commit '{}' not found in repository '{}/{}'",
        ref_name, owner, repo
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for parse_search_pattern function
    #[test]
    fn test_parse_search_pattern_user_with_keyword() {
        let result = parse_search_pattern("rust-lang/compiler");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::UserWithKeyword { username, keyword } => {
                assert_eq!(username, "rust-lang");
                assert_eq!(keyword, "compiler");
            }
            _ => panic!("Expected UserWithKeyword pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_user_all_repos() {
        let result = parse_search_pattern("torvalds/");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::UserAllRepos { username } => {
                assert_eq!(username, "torvalds");
            }
            _ => panic!("Expected UserAllRepos pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_global_keyword() {
        let result = parse_search_pattern("/docker");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::GlobalKeyword { keyword } => {
                assert_eq!(keyword, "docker");
            }
            _ => panic!("Expected GlobalKeyword pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_no_slash_global() {
        let result = parse_search_pattern("kubernetes");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::GlobalKeyword { keyword } => {
                assert_eq!(keyword, "kubernetes");
            }
            _ => panic!("Expected GlobalKeyword pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_empty_string() {
        let result = parse_search_pattern("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_search_pattern_whitespace_only() {
        let result = parse_search_pattern("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_search_pattern_empty_global_keyword() {
        let result = parse_search_pattern("/");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Keyword cannot be empty"));
    }

    #[test]
    fn test_parse_search_pattern_with_leading_trailing_spaces() {
        let result = parse_search_pattern("  rust-lang/compiler  ");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::UserWithKeyword { username, keyword } => {
                assert_eq!(username, "rust-lang");
                assert_eq!(keyword, "compiler");
            }
            _ => panic!("Expected UserWithKeyword pattern"),
        }
    }
}
