use crate::cli::Cli;
use crate::models::Result;
use jlogger_tracing::{jdebug, jinfo};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use std::fs;

/// Add authentication header to request headers
pub fn add_auth_header(cli: &Cli, header: &mut HeaderMap) -> Result<()> {
    let mut success = false;

    // Try direct token first
    if let Some(token) = &cli.token {
        jinfo!("Using token from command line");
        let auth_value = format!("Bearer {}", token.trim());
        header.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).map_err(|e| e.to_string())?,
        );
        success = true;
    } else if let Some(token_file) = &cli.token_file {
        // Try token file
        jinfo!("Using token from file: {}", token_file);
        match fs::read_to_string(token_file) {
            Ok(token) => {
                let auth_value = format!("Bearer {}", token.trim());
                header.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&auth_value).map_err(|e| e.to_string())?,
                );
                success = true;
            }
            Err(e) => {
                return Err(format!("Failed to read token file: {}", e));
            }
        }
    } else {
        // Try .netrc as fallback
        if let Ok(home) = std::env::var("HOME") {
            let netrc_path = std::path::Path::new(&home).join(".netrc");
            jdebug!("Trying .netrc at {:?}", netrc_path);

            if let Ok(content) = fs::read_to_string(&netrc_path) {
                jinfo!("Using .netrc for authentication");
                let lines: Vec<&str> = content.lines().collect();
                let mut in_github = false;

                for line in lines {
                    let trimmed = line.trim();

                    if trimmed.starts_with("machine") {
                        if trimmed.contains("github.com") {
                            jinfo!("Found machine github.com in .netrc");
                            in_github = true;
                        } else {
                            in_github = false;
                        }
                    } else if in_github && trimmed.starts_with("password") {
                        if let Some(password) = trimmed.split_whitespace().nth(1) {
                            let auth_value = format!("Bearer {}", password);
                            header.insert(
                                AUTHORIZATION,
                                HeaderValue::from_str(&auth_value).map_err(|e| e.to_string())?,
                            );
                            success = true;
                            break;
                        }
                    }
                }
            }
        }
    }

    if success {
        Ok(())
    } else {
        Err("No authentication method provided".to_string())
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
    if let Ok(home) = std::env::var("HOME") {
        let netrc_path = std::path::Path::new(&home).join(".netrc");
        if let Ok(content) = std::fs::read_to_string(&netrc_path) {
            let lines: Vec<&str> = content.lines().collect();
            let mut in_github = false;
            for line in lines {
                let trimmed = line.trim();
                if trimmed.starts_with("machine") && trimmed.contains("github.com") {
                    in_github = true;
                } else if in_github && trimmed.starts_with("password") {
                    if let Some(password) = trimmed.split_whitespace().nth(1) {
                        return Some(password.to_string());
                    }
                } else if trimmed.starts_with("machine") {
                    in_github = false;
                }
            }
        }
    }

    None
}
