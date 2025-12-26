use crate::cli::Cli;
use crate::errors::{GhrError, Result};
use jlogger_tracing::{jdebug, jinfo};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use std::fs;

/// Read GitHub token from .netrc file
fn read_netrc_token() -> Option<String> {
    if let Ok(home) = std::env::var("HOME") {
        let netrc_path = std::path::Path::new(&home).join(".netrc");
        jdebug!("Trying .netrc at {:?}", netrc_path);

        if let Ok(content) = std::fs::read_to_string(&netrc_path) {
            return parse_netrc_github_token(&content);
        }
    }
    None
}

/// Parse GitHub token from .netrc file content
fn parse_netrc_github_token(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_github = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("machine") && trimmed.contains("github.com") {
            jinfo!("Found machine github.com in .netrc");
            in_github = true;
        } else if in_github && trimmed.starts_with("password") {
            return trimmed.split_whitespace().nth(1).map(String::from);
        } else if trimmed.starts_with("machine") {
            in_github = false;
        }
    }
    None
}

/// Add authentication header to request headers
pub fn add_auth_header(cli: &Cli, header: &mut HeaderMap) -> Result<()> {
    let mut success = false;

    // Try direct token first
    if let Some(token) = &cli.token {
        jinfo!("Using token from command line");
        let auth_value = format!("Bearer {}", token.trim());
        header.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
        success = true;
    } else if let Some(token_file) = &cli.token_file {
        // Try token file
        jinfo!("Using token from file: {}", token_file);
        match fs::read_to_string(token_file) {
            Ok(token) => {
                let auth_value = format!("Bearer {}", token.trim());
                header.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
                success = true;
            }
            Err(e) => {
                return Err(GhrError::Auth(format!("Failed to read token file: {}", e)));
            }
        }
    } else {
        // Try .netrc as fallback
        if let Some(token) = read_netrc_token() {
            jinfo!("Using .netrc for authentication");
            let auth_value = format!("Bearer {}", token.trim());
            header.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
            success = true;
        }
    }

    if success {
        Ok(())
    } else {
        Err(GhrError::Auth(
            "No authentication method provided".to_string(),
        ))
    }
}

/// Extract token from CLI arguments
pub fn extract_token_from_cli(cli: &Cli) -> Option<String> {
    // Try direct token first
    if let Some(token) = &cli.token {
        return Some(token.clone());
    }

    // Try token file
    if let Some(token_file) = &cli.token_file {
        if let Ok(token) = std::fs::read_to_string(token_file) {
            return Some(token.trim().to_string());
        }
    }

    // Try .netrc
    read_netrc_token()
}
