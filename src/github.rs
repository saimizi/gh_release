use crate::constants;
use crate::errors::{GhrError, Result};
use crate::models::{Release, Repository, RepositoryInfo, SearchResponse};
use jlogger_tracing::{jdebug, jinfo};
use reqwest::Client;
use tokio::time::{sleep, Duration};

/// Retry an async operation with exponential backoff
/// Only retries on network-related errors, not on logical errors like 404
async fn retry_with_backoff<F, T, Fut>(operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let max_retries = constants::retry::MAX_RETRIES;
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Only retry on network errors, not on logical errors
                let should_retry = matches!(e, GhrError::Network(_));

                if should_retry && attempts < max_retries {
                    let delay =
                        Duration::from_secs(constants::retry::BASE_DELAY_SECS * 2u64.pow(attempts));
                    jdebug!("Retry attempt {} after {:?}: {}", attempts + 1, delay, e);
                    sleep(delay).await;
                    attempts += 1;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// Fetch release information from GitHub
pub async fn get_release_info(
    client: &Client,
    repo: &str,
    tag: Option<&str>,
) -> Result<Vec<Release>> {
    // Parse owner/repo from repo string
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err(GhrError::Generic(format!(
            "Invalid repository format: {}",
            repo
        )));
    }
    let (owner, repo_name) = (parts[0], parts[1]);

    let url = if let Some(tag) = tag {
        constants::endpoints::release_by_tag(owner, repo_name, tag)
    } else {
        constants::endpoints::releases(owner, repo_name)
    };

    retry_with_backoff(|| async {
        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(GhrError::GitHubApi(format!(
                "Failed to fetch releases: HTTP {}",
                response.status()
            )));
        }

        if tag.is_some() {
            // Single release
            let release: Release = response.json().await?;
            Ok(vec![release])
        } else {
            // Multiple releases
            let releases: Vec<Release> = response.json().await?;
            Ok(releases)
        }
    })
    .await
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
        return Err(GhrError::InvalidSearchPattern(
            "Search pattern cannot be empty".to_string(),
        ));
    }

    if let Some(slash_pos) = pattern.find('/') {
        let username = &pattern[..slash_pos];
        let keyword = &pattern[slash_pos + 1..];

        if username.is_empty() {
            // Pattern: "/keyword"
            if keyword.is_empty() {
                return Err(GhrError::InvalidSearchPattern(
                    "Keyword cannot be empty for global search".to_string(),
                ));
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

    let url = constants::endpoints::search_repositories(&query, num);

    retry_with_backoff(|| async {
        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(GhrError::GitHubApi(format!(
                "Failed to search repositories: HTTP {}",
                response.status()
            )));
        }

        let search_response: SearchResponse = response.json().await?;

        Ok(search_response.items)
    })
    .await
}

/// Validate that a repository exists and is accessible
pub async fn validate_repository(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<RepositoryInfo> {
    let url = constants::endpoints::repository(owner, repo);

    jinfo!("Validating repository {}/{}...", owner, repo);

    retry_with_backoff(|| async {
        let response = client.get(&url).send().await?;

        if response.status().is_success() {
            let repo_info: RepositoryInfo = response.json().await?;
            Ok(repo_info)
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Err(GhrError::RepositoryNotFound {
                owner: owner.to_string(),
                repo: repo.to_string(),
            })
        } else {
            Err(GhrError::GitHubApi(format!(
                "Failed to validate repository: HTTP {}",
                response.status()
            )))
        }
    })
    .await
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
    let branch_url = constants::endpoints::branch(owner, repo, ref_name);

    let response = retry_with_backoff(|| async {
        client
            .get(&branch_url)
            .send()
            .await
            .map_err(GhrError::Network)
    })
    .await?;

    if response.status().is_success() {
        return Ok("branch".to_string());
    }

    // Try as tag
    let tag_url = constants::endpoints::tag(owner, repo, ref_name);

    let response = retry_with_backoff(|| async {
        client.get(&tag_url).send().await.map_err(GhrError::Network)
    })
    .await?;

    if response.status().is_success() {
        return Ok("tag".to_string());
    }

    // Try as commit SHA
    let commit_url = constants::endpoints::commit(owner, repo, ref_name);

    let response = retry_with_backoff(|| async {
        client
            .get(&commit_url)
            .send()
            .await
            .map_err(GhrError::Network)
    })
    .await?;

    if response.status().is_success() {
        return Ok("commit".to_string());
    }

    // Ref not found
    Err(GhrError::RefNotFound {
        owner: owner.to_string(),
        repo: repo.to_string(),
        ref_name: ref_name.to_string(),
    })
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
        // Check that it's an InvalidSearchPattern error
        matches!(result.unwrap_err(), GhrError::InvalidSearchPattern(_));
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
        // Check that it's an InvalidSearchPattern error
        matches!(result.unwrap_err(), GhrError::InvalidSearchPattern(_));
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
