use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// GitHub release asset
#[derive(Debug, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub download_count: u32,
}

impl Display for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size_mb = self.size as f64 / 1_048_576.0;
        write!(
            f,
            "  - {} ({:.2} MB, {} downloads)",
            self.name, size_mb, self.download_count
        )
    }
}

/// GitHub release
#[derive(Debug, Deserialize, Serialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub published_at: String,
    pub assets: Vec<Asset>,
    pub body: Option<String>,
}

impl Display for Release {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name.as_deref().unwrap_or("N/A");
        writeln!(f, "Tag: {}", self.tag_name)?;
        writeln!(f, "Name: {}", name)?;
        writeln!(f, "Published: {}", self.published_at)?;
        writeln!(f, "Assets:")?;
        for asset in &self.assets {
            writeln!(f, "{}", asset)?;
        }
        Ok(())
    }
}

/// Search response from GitHub API
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub items: Vec<Repository>,
}

/// GitHub repository
#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Repository {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub stargazers_count: u32,
    pub html_url: String,
    pub owner: Owner,
    pub private: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Owner {
    pub login: String,
}

impl Repository {
    pub fn summary(&self) -> String {
        // Add lock emoji for private repositories
        let privacy_indicator = if self.private { "*" } else { " " };

        format!(
            "{:<7} {:2}{:40}",
            self.stargazers_count, privacy_indicator, self.full_name
        )
    }
}

impl Display for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let desc = self.description.as_deref().unwrap_or("");

        // Truncate description respecting UTF-8 character boundaries
        let desc_truncated = if desc.chars().count() > 50 {
            let truncated: String = desc.chars().take(47).collect();
            format!("{}...", truncated)
        } else {
            desc.to_string()
        };

        // Add lock emoji for private repositories
        let privacy_indicator = if self.private { "*" } else { " " };

        let msg = format!(
            "{:<7} {:2}{:40} {:52}",
            self.stargazers_count, privacy_indicator, self.full_name, desc_truncated
        );

        write!(f, "{}", msg)
    }
}

/// Clone specification parsed from user input
#[derive(Debug)]
pub struct CloneSpec {
    pub owner: String,
    pub repo: String,
    pub ref_name: Option<String>,
    pub original_url: String,
}

/// Repository info from GitHub API
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RepositoryInfo {
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub private: bool,
}

// Result type is now defined in errors.rs

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for Repository methods
    #[test]
    fn test_repository_summary_public_repo() {
        let repo = Repository {
            name: "test-repo".to_string(),
            full_name: "user/test-repo".to_string(),
            description: Some("A test repository".to_string()),
            stargazers_count: 42,
            html_url: "https://github.com/user/test-repo".to_string(),
            owner: Owner {
                login: "user".to_string(),
            },
            private: false,
        };

        let summary = repo.summary();
        assert!(summary.contains("user/test-repo"));
        assert!(summary.contains("42"));
        assert!(!summary.contains("*")); // Not private
    }

    #[test]
    fn test_repository_summary_private_repo() {
        let repo = Repository {
            name: "private-repo".to_string(),
            full_name: "org/private-repo".to_string(),
            description: Some("A private repository".to_string()),
            stargazers_count: 100,
            html_url: "https://github.com/org/private-repo".to_string(),
            owner: Owner {
                login: "org".to_string(),
            },
            private: true,
        };

        let summary = repo.summary();
        assert!(summary.contains("org/private-repo"));
        assert!(summary.contains("100"));
        assert!(summary.contains("*")); // Private indicator
    }

    #[test]
    fn test_repository_summary_zero_stars() {
        let repo = Repository {
            name: "new-repo".to_string(),
            full_name: "user/new-repo".to_string(),
            description: None,
            stargazers_count: 0,
            html_url: "https://github.com/user/new-repo".to_string(),
            owner: Owner {
                login: "user".to_string(),
            },
            private: false,
        };

        let summary = repo.summary();
        assert!(summary.contains("user/new-repo"));
        assert!(summary.contains("0"));
    }

    #[test]
    fn test_repository_summary_high_star_count() {
        let repo = Repository {
            name: "popular-repo".to_string(),
            full_name: "org/popular-repo".to_string(),
            description: Some("Very popular".to_string()),
            stargazers_count: 123456,
            html_url: "https://github.com/org/popular-repo".to_string(),
            owner: Owner {
                login: "org".to_string(),
            },
            private: false,
        };

        let summary = repo.summary();
        assert!(summary.contains("org/popular-repo"));
        assert!(summary.contains("123456"));
    }
}
