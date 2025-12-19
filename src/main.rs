use chrono::prelude::*;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[allow(unused_imports)]
use {
    clap::{ArgAction, Parser},
    futures::stream::{self, StreamExt},
    indicatif::{MultiProgress, ProgressBar, ProgressStyle},
    jlogger_tracing::{jdebug, jerror, jinfo, jwarn, JloggerBuilder, LevelFilter, LogTimeFormat},
    reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT},
    reqwest::Client,
    serde::Deserialize,
    std::fs,
    std::path::PathBuf,
    std::sync::Arc,
};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Asset {
    name: Option<String>,
    url: Option<String>, // API endpoint for downloading (works with authentication)
    browser_download_url: Option<String>,
    size: Option<u64>,
    download_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Release {
    name: Option<String>,
    tag_name: Option<String>,
    published_at: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
    assets: Vec<Asset>,
}

impl Release {
    // Additional methods can be added here if needed
    pub fn summary(&self) -> String {
        let types = if self.draft.unwrap_or(false) {
            "Draft"
        } else if self.prerelease.unwrap_or(false) {
            "Pre"
        } else {
            "Rel"
        };

        let published_at = self
            .published_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .and_then(|dt| {
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
                    .into()
            })
            .unwrap_or_else(|| "Unknown".to_string());

        format!(
            "{:15} {:15} {:5} {:20} {:4}",
            self.name.as_deref().unwrap_or("Unnamed"),
            self.tag_name.as_deref().unwrap_or("No tag"),
            types,
            published_at,
            self.assets.len()
        )
    }
}

impl Display for Release {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let draft = self
            .draft
            .map(|d| if d { "Draft" } else { "" })
            .unwrap_or("");
        let prerelease = self
            .prerelease
            .map(|d| if d { "Pre" } else { "" })
            .unwrap_or("");
        writeln!(
            f,
            "Release: {} [{} {}]",
            self.name.as_deref().unwrap_or("Unnamed"),
            draft,
            prerelease
        )?;
        writeln!(f, "Tag: {}", self.tag_name.as_deref().unwrap_or("No tag"))?;
        if let Some(dt) = self
            .published_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        {
            writeln!(
                f,
                "Published at: {}",
                dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")
            )?;
        }
        writeln!(f, "Assets:")?;
        for asset in &self.assets {
            writeln!(
                f,
                "  {} size: {} download count: {}",
                asset.name.as_deref().unwrap_or("Unnamed"),
                asset.size.unwrap_or(0),
                asset.download_count.unwrap_or(0)
            )?;
        }
        Ok(())
    }
}

type Result<T> = std::result::Result<T, String>;

/// CLI arguments
#[derive(Parser)]
#[command(
    name = "Github release fetcher",
    version,
    about = "A tool to retrieve and download github release package."
)]
struct Cli {
    /// GitHub Repository in the format "owner/repo"
    #[arg(long, short = 'r')]
    repo: String,

    /// Token for GitHub API authentication
    #[arg(short = 't', long = "token")]
    token: Option<String>,

    /// File containing GitHub API token
    #[arg(short = 'T', long = "token-file")]
    token_file: Option<String>,

    /// Specific version to download (or "latest" for the most recent release)
    #[arg(short = 'd', long = "download")]
    download: Option<String>,

    /// String used to filter the name of assets to download, multiple filters can be separated by
    /// commas.
    #[arg(short = 'f', long = "filter")]
    filter: Option<String>,

    /// Directory to save downloaded assets (defaults to current directory)
    #[arg(short = 'o', long = "output-dir")]
    output_dir: Option<PathBuf>,

    /// Show information about a specific version, multiple versions can be separated by commas.
    #[arg(short = 'i', long = "info")]
    info: Option<String>,

    /// Number of packages to fetch
    #[arg(short = 'n', long = "num", default_value_t = 10)]
    num: usize,

    /// Maximum number of concurrent downloads
    #[arg(short = 'c', long = "concurrency", default_value_t = 5)]
    concurrency: usize,

    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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

    if add_auth_header(&cli, &mut header).is_err() {
        jinfo!("No authentication method provided, proceeding unauthenticated");
    }

    let client = Client::builder()
        .default_headers(header)
        .build()
        .map_err(|e| e.to_string())?;

    if let Some(download) = cli.download.as_deref() {
        let releases = get_release_info(&client, &cli.repo, None).await?;

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
                .find(|r| r.tag_name.as_deref() == Some(download))
                .ok_or_else(|| format!("Release with tag '{}' not found", download))?
        };

        // Create output directory if specified
        if let Some(output_dir) = &cli.output_dir {
            fs::create_dir_all(output_dir).map_err(|e| {
                format!(
                    "Failed to create output directory '{}': {}",
                    output_dir.display(),
                    e
                )
            })?;
            jinfo!("Saving assets to: {}", output_dir.display());
        }

        // Collect assets to download with filtering
        let mut assets_to_download = Vec::new();
        for asset in &release.assets {
            if let Some(name) = &asset.name {
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
                let download_url = asset
                    .url
                    .as_ref()
                    .or(asset.browser_download_url.as_ref())
                    .ok_or_else(|| format!("No download URL available for asset '{}'", name))?;

                // Get asset size for progress bar
                let size = asset.size.unwrap_or(0);

                // Construct output path
                let output_path = if let Some(output_dir) = &cli.output_dir {
                    output_dir.join(name)
                } else {
                    PathBuf::from(name)
                };

                assets_to_download.push((name.clone(), download_url.clone(), output_path, size));
            }
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
                        pb.finish_with_message(format!("❌ Failed: {} (HTTP {})", name, status));
                        return Err(format!("HTTP {} for '{}'", status, name));
                    }

                    // Read bytes with progress
                    let mut downloaded: u64 = 0;
                    let mut bytes_vec = Vec::new();
                    let mut stream = response.bytes_stream();

                    while let Some(chunk_result) = stream.next().await {
                        let chunk = chunk_result
                            .map_err(|e| format!("Failed to read chunk for '{}': {}", name, e))?;
                        bytes_vec.extend_from_slice(&chunk);
                        downloaded += chunk.len() as u64;
                        pb.set_position(downloaded);
                    }

                    // Write to file
                    fs::write(&output_path, &bytes_vec)
                        .map_err(|e| format!("Failed to save '{}': {}", output_path.display(), e))?;

                    pb.finish_with_message(format!("✓ Downloaded: {}", name));
                    Ok(format!("Successfully downloaded: {}", output_path.display()))
                }
            })
            .buffer_unordered(cli.concurrency) // Limit concurrent downloads
            .collect()
            .await;

        // Check results and report errors
        let mut success_count = 0;
        let mut failed_downloads = Vec::new();

        for result in download_results {
            match result {
                Ok(msg) => {
                    jinfo!("{}", msg);
                    success_count += 1;
                }
                Err(e) => {
                    jerror!("{}", e);
                    failed_downloads.push(e);
                }
            }
        }

        jinfo!(
            "Download complete: {} succeeded, {} failed",
            success_count,
            failed_downloads.len()
        );

        // Return error if any downloads failed (but after attempting all)
        if !failed_downloads.is_empty() {
            return Err(format!(
                "Failed to download {} asset(s): {}",
                failed_downloads.len(),
                failed_downloads.join(", ")
            ));
        }
        return Ok(());
    }
    if let Some(info) = cli.info.as_deref() {
        let versions = info.split(',').collect::<Vec<&str>>();

        let releases = get_release_info(&client, &cli.repo, None).await?;

        for ver in versions {
            let release = releases
                .iter()
                .find(|r| r.tag_name.as_deref() == Some(ver))
                .ok_or_else(|| format!("Release with tag '{}' not found", ver))?;
            eprintln!("{}", release);
            eprintln!("---------------------");
        }
    } else {
        let releases = get_release_info(&client, &cli.repo, Some(cli.num)).await?;
        eprintln!(
            "{:4} {:15} {:15} {:5} {:20} {:4}",
            "No", "Name", "Tag", "Type", "Published", "Assets"
        );
        for (i, r) in releases.iter().enumerate() {
            eprintln!("{:<4} {}", i + 1, r.summary());
        }
    }

    Ok(())
}

async fn get_release_info(client: &Client, repo: &str, num: Option<usize>) -> Result<Vec<Release>> {
    let mut url = format!("https://api.github.com/repos/{}/releases", repo.trim());
    if let Some(num) = num {
        url = format!(
            "https://api.github.com/repos/{}/releases?per_page={}&page=1",
            repo.trim(),
            num
        );
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub API request failed with status: {}", status));
    }

    let releases: Vec<Release> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    Ok(releases)
}

fn add_auth_header(cli: &Cli, header: &mut HeaderMap) -> Result<()> {
    let mut success = false;
    if let Some(token) = cli.token.as_deref() {
        jinfo!("Using provided token for authentication");
        let auth_value = format!("Bearer {}", token);
        header.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).map_err(|e| e.to_string())?,
        );
        success = true;
    } else if let Some(token_file) = cli.token_file.as_deref() {
        jinfo!("Using token file '{}' for authentication", token_file);
        let path = PathBuf::from(token_file);
        let token = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read token file '{}': {}", path.display(), e))?;
        let token = token.trim();
        let auth_value = format!("Bearer {}", token);
        header.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).map_err(|e| e.to_string())?,
        );

        success = true;
    } else if let Ok(netrc) = File::open(dirs::home_dir().unwrap().join(".netrc")) {
        jinfo!("Using .netrc for authentication");
        let reader = BufReader::new(netrc);
        let mut in_github_block = false;
        for l in reader.lines().map_while(|r| r.ok()) {
            // Search for the first machine block for github.com
            // Note if there are multiple blocks only the first is used
            if l.trim().starts_with("machine ") && l.ends_with("github.com") {
                in_github_block = true;
                jinfo!(
                    "Found machine {} in .netrc",
                    l.replace("machine ", "").trim()
                );
            } else if l.trim().starts_with("machine ") {
                in_github_block = false;
            }

            if l.trim().starts_with("password ") && in_github_block {
                if let Some(password) = l.split_whitespace().nth(1) {
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

    if success {
        Ok(())
    } else {
        Err("No authentication method provided".to_string())
    }
}
