use crate::cli::Cli;
use crate::models::{CloneSpec, Result};
use jlogger_tracing::{jdebug, jinfo, jwarn};

/// Parse clone URL and extract owner, repo, and optional ref
pub fn parse_clone_url(url: &str) -> Result<CloneSpec> {
    let url = url.trim();

    if url.is_empty() {
        return Err("Clone URL cannot be empty".to_string());
    }

    // Split by ':' to separate URL and optional ref
    let parts: Vec<&str> = url.splitn(2, ':').collect();
    let (url_part, ref_name) = if parts.len() == 2 {
        // Check if this is an SSH URL (contains '@') or HTTPS with ref
        if parts[0].contains('@') || parts[0].starts_with("https") || parts[0].starts_with("http") {
            // This is a full URL, not a ref separator
            (url, None)
        } else {
            // This is URL:ref format (e.g., owner/repo:branch)
            (parts[0], Some(parts[1].to_string()))
        }
    } else {
        (url, None)
    };

    // Extract owner and repo from URL
    let (owner, repo) = if url_part.starts_with("https://github.com/")
        || url_part.starts_with("http://github.com/")
    {
        // HTTPS URL: https://github.com/owner/repo or https://github.com/owner/repo.git
        let path = url_part
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/")
            .trim_end_matches(".git");

        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 2 {
            return Err(format!("Invalid GitHub URL: {}", url_part));
        }
        (parts[0].to_string(), parts[1].to_string())
    } else if url_part.starts_with("git@github.com:") {
        // SSH URL: git@github.com:owner/repo.git
        let path = url_part
            .trim_start_matches("git@github.com:")
            .trim_end_matches(".git");

        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 2 {
            return Err(format!("Invalid GitHub SSH URL: {}", url_part));
        }
        (parts[0].to_string(), parts[1].to_string())
    } else if url_part.contains('/') {
        // Short format: owner/repo
        let parts: Vec<&str> = url_part.split('/').collect();
        if parts.len() < 2 {
            return Err(format!("Invalid repository format: {}", url_part));
        }
        (
            parts[0].to_string(),
            parts[1].trim_end_matches(".git").to_string(),
        )
    } else {
        return Err(format!(
            "Unsupported URL format: {}. Use 'owner/repo', 'https://github.com/owner/repo', or 'git@github.com:owner/repo.git'",
            url_part
        ));
    };

    if owner.is_empty() || repo.is_empty() {
        return Err("Owner and repository name cannot be empty".to_string());
    }

    Ok(CloneSpec {
        owner,
        repo,
        ref_name,
        original_url: url_part.to_string(),
    })
}

/// Extract repository name from URL for default directory name
pub fn get_repo_name(url: &str) -> String {
    // Try to parse the URL first
    if let Ok(spec) = parse_clone_url(url) {
        return spec.repo;
    }

    // Fallback: extract from URL manually
    let url = url.trim().trim_end_matches(".git");

    if let Some(last_part) = url.split('/').next_back() {
        if !last_part.is_empty() {
            return last_part.to_string();
        }
    }

    // Final fallback
    "cloned-repo".to_string()
}

/// Check if git is installed and available in PATH
pub fn check_git_installed() -> Result<()> {
    let output = std::process::Command::new("git").arg("--version").output();

    match output {
        Ok(output) if output.status.success() => {
            jdebug!(
                "Git version: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
            Ok(())
        }
        Ok(_) => Err("Git command failed. Please ensure git is properly installed.".to_string()),
        Err(_) => Err(
            "Git is not installed or not in PATH. Please install git to use the clone feature."
                .to_string(),
        ),
    }
}

/// Construct clone URL with optional authentication
pub fn construct_clone_url(owner: &str, repo: &str, token: Option<&str>) -> String {
    if let Some(token) = token {
        format!("https://{}@github.com/{}/{}.git", token, owner, repo)
    } else {
        format!("https://github.com/{}/{}.git", owner, repo)
    }
}

/// Execute git clone command
pub fn execute_git_clone(clone_url: &str, target_dir: &str, ref_name: Option<&str>) -> Result<()> {
    // Check target directory doesn't exist
    if std::path::Path::new(target_dir).exists() {
        return Err(format!(
            "Directory '{}' already exists. Please remove it or choose a different name.",
            target_dir
        ));
    }

    // Execute git clone
    jinfo!("Executing: git clone <url> {}", target_dir);
    let output = std::process::Command::new("git")
        .arg("clone")
        .arg(clone_url)
        .arg(target_dir)
        .output()
        .map_err(|e| format!("Failed to execute git clone: {}", e))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        cleanup_partial_clone(target_dir);
        return Err(format!("Git clone failed: {}", error.trim()));
    }

    // Show git output
    if !output.stdout.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    // Checkout specific ref if provided
    if let Some(ref_name) = ref_name {
        jinfo!("Checking out ref '{}'...", ref_name);
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(target_dir)
            .arg("checkout")
            .arg(ref_name)
            .output()
            .map_err(|e| format!("Failed to execute git checkout: {}", e))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            cleanup_partial_clone(target_dir);
            return Err(format!("Git checkout failed: {}", error.trim()));
        }

        if !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    Ok(())
}

/// Attempt to cleanup partial clone on failure
pub fn cleanup_partial_clone(dir: &str) {
    jinfo!("Attempting to cleanup partial clone at '{}'...", dir);
    if let Err(e) = std::fs::remove_dir_all(dir) {
        jwarn!("Failed to cleanup directory '{}': {}", dir, e);
        jwarn!("Please manually remove the directory if it exists.");
    } else {
        jinfo!("Cleanup successful.");
    }
}

/// Extract token from CLI arguments (for git clone)
pub fn extract_token_for_clone(cli: &Cli) -> Option<String> {
    crate::auth::extract_token_from_cli(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for parse_clone_url function
    #[test]
    fn test_parse_clone_url_https() {
        let spec = parse_clone_url("https://github.com/owner/repo").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, None);
    }

    #[test]
    fn test_parse_clone_url_https_with_git() {
        let spec = parse_clone_url("https://github.com/owner/repo.git").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, None);
    }

    #[test]
    fn test_parse_clone_url_ssh() {
        let spec = parse_clone_url("git@github.com:owner/repo.git").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, None);
    }

    #[test]
    fn test_parse_clone_url_short_format() {
        let spec = parse_clone_url("owner/repo").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, None);
    }

    #[test]
    fn test_parse_clone_url_with_ref() {
        let spec = parse_clone_url("owner/repo:main").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, Some("main".to_string()));
    }

    #[test]
    fn test_parse_clone_url_with_branch() {
        let spec = parse_clone_url("saimizi/gh_release:feature/new-feature").unwrap();
        assert_eq!(spec.owner, "saimizi");
        assert_eq!(spec.repo, "gh_release");
        assert_eq!(spec.ref_name, Some("feature/new-feature".to_string()));
    }

    #[test]
    fn test_parse_clone_url_with_tag() {
        let spec = parse_clone_url("owner/repo:v1.2.3").unwrap();
        assert_eq!(spec.owner, "owner");
        assert_eq!(spec.repo, "repo");
        assert_eq!(spec.ref_name, Some("v1.2.3".to_string()));
    }

    #[test]
    fn test_parse_clone_url_invalid_empty() {
        let result = parse_clone_url("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_clone_url_invalid_format() {
        let result = parse_clone_url("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repo_name_https() {
        assert_eq!(get_repo_name("https://github.com/owner/my-repo"), "my-repo");
    }

    #[test]
    fn test_get_repo_name_https_with_git() {
        assert_eq!(get_repo_name("https://github.com/owner/repo.git"), "repo");
    }

    #[test]
    fn test_get_repo_name_short() {
        assert_eq!(get_repo_name("owner/my-repo"), "my-repo");
    }

    #[test]
    fn test_get_repo_name_ssh() {
        assert_eq!(get_repo_name("git@github.com:owner/repo.git"), "repo");
    }

    #[test]
    fn test_get_repo_name_with_ref() {
        assert_eq!(get_repo_name("owner/repo:main"), "repo");
    }

    #[test]
    fn test_construct_clone_url() {
        let url = construct_clone_url("owner", "repo", Some("token123"));
        assert_eq!(url, "https://token123@github.com/owner/repo.git");

        let url = construct_clone_url("owner", "repo", None);
        assert_eq!(url, "https://github.com/owner/repo.git");
    }
}
