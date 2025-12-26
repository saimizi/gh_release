mod auth;
mod cli;
mod git;
mod github;
mod models;

use chrono::prelude::*;
use cli::Cli;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use jlogger_tracing::{jdebug, jerror, jinfo, JloggerBuilder, LevelFilter, LogTimeFormat};
use models::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate that either --repo, --search, or --clone is provided
    if cli.repo.is_none() && cli.search.is_none() && cli.clone.is_none() {
        return Err(
            "Either --repo, --search, or --clone must be provided. Use --help for more information."
                .to_string(),
        );
    }

    let verbose = cli.verbose;
    let log_level = match verbose {
        1 => LevelFilter::DEBUG,
        2 => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    };

    JloggerBuilder::new()
        .max_level(log_level)
        .log_console(true)
        .log_time(LogTimeFormat::TimeLocal)
        .build();

    let mut header = HeaderMap::new();

    header.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github.v3+json"),
    );
    header.insert(USER_AGENT, HeaderValue::from_static("gh_release"));
    header.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );

    if auth::add_auth_header(&cli, &mut header).is_err() {
        jinfo!("No authentication method provided, proceeding unauthenticated");
    }

    let client = Client::builder()
        .default_headers(header)
        .build()
        .map_err(|e| e.to_string())?;

    // CLONE MODE - handle repository cloning
    if let Some(clone_arg) = cli.clone.as_deref() {
        jinfo!("Clone mode activated");

        // Check git is installed
        git::check_git_installed()?;

        // Parse clone specification
        let spec = git::parse_clone_url(clone_arg)?;
        jinfo!("Cloning repository: {}/{}", spec.owner, spec.repo);

        // Validate repository exists
        let repo_info = github::validate_repository(&client, &spec.owner, &spec.repo).await?;
        jinfo!(
            "Repository found: {} ({})",
            repo_info.full_name,
            if repo_info.private {
                "private"
            } else {
                "public"
            }
        );

        // Validate ref if specified
        if let Some(ref_name) = spec.ref_name.as_ref() {
            let ref_type = github::validate_ref(&client, &spec.owner, &spec.repo, ref_name).await?;
            jinfo!("Reference '{}' found (type: {})", ref_name, ref_type);
        }

        // Determine target directory
        let default_dir = git::get_repo_name(&spec.original_url);
        let target_dir = cli.directory.as_deref().unwrap_or(&default_dir);

        // Extract token from CLI for clone URL
        let token = git::extract_token_for_clone(&cli);

        // Construct clone URL with auth if available
        let clone_url = git::construct_clone_url(&spec.owner, &spec.repo, token.as_deref());

        // Execute clone
        jinfo!("Cloning to '{}'...", target_dir);
        git::execute_git_clone(&clone_url, target_dir, spec.ref_name.as_deref())?;

        jinfo!("Successfully cloned repository to '{}'", target_dir);
        return Ok(());
    }

    // SEARCH MODE - handle repository search
    if let Some(search_pattern) = cli.search.as_deref() {
        jinfo!("Searching repositories with pattern: {}", search_pattern);

        let pattern = github::parse_search_pattern(search_pattern)?;
        let repositories = github::search_repositories(&client, &pattern, cli.num).await?;

        if repositories.is_empty() {
            jinfo!("No repositories found matching the search criteria");
            return Ok(());
        }

        // Display results in table format
        eprintln!("{:4} {:<7} {:2}{:40}", "No", "Stars", " ", "Repository",);
        eprintln!("{:-<108}", "");

        for (i, repo) in repositories.iter().enumerate() {
            eprintln!("{:<4} {}", i + 1, repo.summary());
        }

        eprintln!("\nFound {} repositories", repositories.len());

        return Ok(());
    }

    if let Some(download) = cli.download.as_deref() {
        let repo = cli
            .repo
            .as_deref()
            .ok_or_else(|| "--repo is required for download mode".to_string())?;
        let releases = github::get_release_info(&client, repo, None).await?;

        // Support "latest" as a special keyword to download the most recent release
        let release = if download == "latest" {
            jinfo!("Downloading latest release");
            releases
                .first()
                .ok_or_else(|| "No releases found in repository".to_string())?
        } else {
            jinfo!("Downloading release: {}", download);
            releases
                .iter()
                .find(|r| r.tag_name == download)
                .ok_or_else(|| format!("Release with tag '{}' not found", download))?
        };

        // Create output directory if specified
        if let Some(directory) = &cli.directory {
            fs::create_dir_all(directory)
                .map_err(|e| format!("Failed to create output directory '{}': {}", directory, e))?;
            jinfo!("Saving assets to: {}", directory);
        }

        // Collect assets to download with filtering
        let mut assets_to_download = Vec::new();
        for asset in &release.assets {
            let name = &asset.name;
            let mut do_download = true;
            if let Some(filter) = cli.filter.as_deref() {
                do_download = false;
                let filters = filter.split(',').collect::<Vec<&str>>();
                for &f in filters.iter() {
                    if name.contains(f) {
                        do_download = true;
                        break;
                    }
                }
            }

            if !do_download {
                jinfo!("Skipping asset '{}' due to filter", name);
                continue;
            }

            // Get download URL
            let download_url = &asset.browser_download_url;

            // Get asset size for progress bar
            let size = asset.size;

            // Construct output path
            let output_path = if let Some(directory) = &cli.directory {
                PathBuf::from(directory).join(name)
            } else {
                PathBuf::from(name)
            };

            assets_to_download.push((name.clone(), download_url.clone(), output_path, size));
        }

        if assets_to_download.is_empty() {
            jinfo!("No assets to download");
            return Ok(());
        }

        jinfo!(
            "Downloading {} asset(s) with concurrency limit of {}",
            assets_to_download.len(),
            cli.concurrency
        );

        // Setup multi-progress bar
        let multi_progress = Arc::new(MultiProgress::new());
        let client = Arc::new(client);

        // Parallel download with concurrency limit
        let download_results: Vec<Result<String>> = stream::iter(assets_to_download)
            .map(|(name, url, output_path, size)| {
                let client = Arc::clone(&client);
                let multi_progress = Arc::clone(&multi_progress);

                async move {
                    // Create progress bar for this asset
                    let pb = multi_progress.add(ProgressBar::new(size));
                    pb.set_style(
                        ProgressStyle::default_bar()
                            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                            .unwrap()
                            .progress_chars("#>-"),
                    );
                    pb.set_message(format!("Downloading: {}", name));

                    jdebug!("Download URL: {}", url);

                    // Download with progress tracking
                    let response = client
                        .get(&url)
                        .header(ACCEPT, "application/octet-stream")
                        .send()
                        .await
                        .map_err(|e| format!("Failed to download '{}': {}", name, e))?;

                    let status = response.status();
                    if !status.is_success() {
                        pb.finish_with_message(format!("Failed: {} (HTTP {})", name, status));
                        return Err(format!("HTTP {} for '{}'", status, name));
                    }

                    // Read bytes with progress
                    let mut downloaded: u64 = 0;
                    let mut bytes_vec = Vec::new();
                    let mut stream = response.bytes_stream();

                    while let Some(chunk_result) = stream.next().await {
                        let chunk =
                            chunk_result.map_err(|e| format!("Download error for '{}': {}", name, e))?;
                        downloaded += chunk.len() as u64;
                        bytes_vec.extend_from_slice(&chunk);
                        pb.set_position(downloaded);
                    }

                    pb.finish_with_message(format!("Complete: {}", name));

                    // Write to file
                    fs::write(&output_path, &bytes_vec).map_err(|e| {
                        format!("Failed to write file '{}': {}", output_path.display(), e)
                    })?;

                    Ok(name)
                }
            })
            .buffer_unordered(cli.concurrency)
            .collect()
            .await;

        // Check for errors
        let mut errors = Vec::new();
        let mut successes = Vec::new();

        for result in download_results {
            match result {
                Ok(name) => successes.push(name),
                Err(e) => errors.push(e),
            }
        }

        // Report results
        if !successes.is_empty() {
            jinfo!("Successfully downloaded {} asset(s)", successes.len());
        }

        if !errors.is_empty() {
            jerror!("Failed to download {} asset(s):", errors.len());
            for error in &errors {
                jerror!("  - {}", error);
            }
            return Err(format!("Download failed with {} error(s)", errors.len()));
        }

        return Ok(());
    }

    // INFO MODE or default list mode
    let repo = cli
        .repo
        .as_deref()
        .ok_or_else(|| "--repo is required for info/list mode".to_string())?;

    if let Some(info_tags) = cli.info.as_deref() {
        // INFO MODE - show detailed information about specific versions
        let tags: Vec<&str> = info_tags.split(',').map(|s| s.trim()).collect();

        for tag in tags {
            jinfo!("Fetching information for release: {}", tag);
            let releases = github::get_release_info(&client, repo, Some(tag)).await?;

            if let Some(release) = releases.first() {
                println!("\n{}", "=".repeat(80));
                println!("{}", release);
                if let Some(body) = &release.body {
                    println!("\nRelease Notes:");
                    println!("{}", "-".repeat(80));
                    println!("{}", body);
                }
                println!("{}", "=".repeat(80));
            }
        }
    } else {
        // LIST MODE - show list of recent releases
        let releases = github::get_release_info(&client, repo, None).await?;
        let releases_to_show = releases.iter().take(cli.num);

        eprintln!(
            "{:4} {:20} {:30} {:15} {:10}",
            "No", "Tag", "Name", "Published", "Assets"
        );
        eprintln!("{:-<108}", "");

        for (i, release) in releases_to_show.enumerate() {
            let name = release.name.as_deref().unwrap_or("N/A");

            // Parse and format the published date
            let published = DateTime::parse_from_rfc3339(&release.published_at)
                .ok()
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            eprintln!(
                "{:<4} {:20} {:30} {:15} {:10}",
                i + 1,
                release.tag_name,
                truncate(name, 30),
                published,
                release.assets.len()
            );
        }

        eprintln!(
            "\nShowing {} of {} releases",
            cli.num.min(releases.len()),
            releases.len()
        );
    }

    Ok(())
}

/// Truncate string to specified length with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}
