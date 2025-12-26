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
    created_at: Option<String>,
    published_at: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
    body: Option<String>,
    assets: Vec<Asset>,
}

impl Release {
    pub fn date_string(date: &str) -> Option<String> {
        if let Ok(dt) = DateTime::parse_from_rfc3339(date) {
            Some(
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            )
        } else {
            None
        }
    }
    fn date_info(&self) -> String {
        if let Some(published_at) = Release::date_string(self.published_at.as_deref().unwrap_or(""))
        {
            published_at
        } else if let Some(created_at) =
            Release::date_string(self.created_at.as_deref().unwrap_or(""))
        {
            created_at
        } else {
            "Unknown".to_string()
        }
    }
    // Additional methods can be added here if needed
    pub fn summary(&self) -> String {
        let types = if self.draft.unwrap_or(false) {
            "Draft"
        } else if self.prerelease.unwrap_or(false) {
            "Pre"
        } else {
            "Rel"
        };

        let name_len = usize::min(15, self.name.as_deref().unwrap_or("Unnamed").len());
        let tag_len = usize::min(15, self.tag_name.as_deref().unwrap_or("Unnamed").len());
        format!(
            "{:15} {:15} {:5} {:20} {:4}",
            &self.name.as_deref().unwrap_or("Unnamed")[..name_len],
            &self.tag_name.as_deref().unwrap_or("No tag")[..tag_len],
            types,
            self.date_info(),
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
            "{:<12}: {} [{} {}]",
            "Release",
            self.name.as_deref().unwrap_or("Unnamed"),
            draft,
            prerelease
        )?;
        writeln!(
            f,
            "{:<12}: {}",
            "Tag",
            self.tag_name.as_deref().unwrap_or("No tag")
        )?;
        writeln!(
            f,
            "{:<12}: {}",
            "Created",
            Release::date_string(self.created_at.as_deref().unwrap_or("-"))
                .unwrap_or("-".to_string())
        )?;

        writeln!(
            f,
            "{:<12}: {}",
            "Published",
            Release::date_string(self.published_at.as_deref().unwrap_or("-"))
                .unwrap_or("-".to_string())
        )?;

        // Display release notes if available
        if let Some(body) = &self.body {
            if !body.is_empty() {
                writeln!(f, "\nRelease Notes:")?;
                writeln!(f, "{}", body)?;
            }
        }

        writeln!(f, "\nAssets:")?;
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SearchResponse {
    total_count: usize,
    incomplete_results: bool,
    items: Vec<Repository>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
    full_name: String,
    description: Option<String>,
    stargazers_count: u32,
    html_url: String,
    owner: Owner,
    private: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Owner {
    login: String,
}

impl Repository {
    pub fn summary(&self) -> String {
        // Add lock emoji for private repositories
        //let privacy_indicator = if self.private { "🔒" } else { "  " };
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
        //let privacy_indicator = if self.private { "🔒" } else { "  " };
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
struct CloneSpec {
    owner: String,
    repo: String,
    ref_name: Option<String>,
    original_url: String,
}

/// Repository info from GitHub API
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RepositoryInfo {
    name: String,
    full_name: String,
    default_branch: String,
    private: bool,
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
    /// GitHub Repository in the format "owner/repo" (required for release operations)
    #[arg(long, short = 'r')]
    repo: Option<String>,

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

    /// Search for repositories using pattern:
    /// - "username/keyword": Search repos owned by username containing keyword
    /// - "username/": List all repos owned by username
    /// - "/keyword": Search top N repos globally containing keyword
    #[arg(short = 's', long = "search")]
    search: Option<String>,

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
    #[arg(short = 'j', long = "concurrency", default_value_t = 5)]
    concurrency: usize,

    /// Clone a repository with optional ref (branch/tag/sha1)
    /// Format: <url>[:<ref>] where url can be:
    ///   - https://github.com/owner/repo
    ///   - git@github.com:owner/repo.git
    ///   - owner/repo (short format)
    #[arg(short = 'c', long = "clone", value_name = "URL[:REF]")]
    clone: Option<String>,

    /// Local directory for cloned repository (defaults to repository name)
    #[arg(value_name = "DIRECTORY", requires = "clone")]
    directory: Option<String>,

    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,
}

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

    if add_auth_header(&cli, &mut header).is_err() {
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
        check_git_installed()?;

        // Parse clone specification
        let spec = parse_clone_url(clone_arg)?;
        jinfo!("Cloning repository: {}/{}", spec.owner, spec.repo);

        // Validate repository exists
        let repo_info = validate_repository(&client, &spec.owner, &spec.repo).await?;
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
            let ref_type = validate_ref(&client, &spec.owner, &spec.repo, ref_name).await?;
            jinfo!("Reference '{}' found (type: {})", ref_name, ref_type);
        }

        // Determine target directory
        let default_dir = get_repo_name(&spec.original_url);
        let target_dir = cli.directory.as_deref().unwrap_or(&default_dir);

        // Extract token from CLI for clone URL
        let token = extract_token_from_cli(&cli);

        // Construct clone URL with auth if available
        let clone_url = construct_clone_url(&spec.owner, &spec.repo, token.as_deref());

        // Execute clone
        jinfo!("Cloning to '{}'...", target_dir);
        execute_git_clone(&clone_url, target_dir, spec.ref_name.as_deref())?;

        jinfo!("Successfully cloned repository to '{}'", target_dir);
        return Ok(());
    }

    // SEARCH MODE - handle repository search
    if let Some(search_pattern) = cli.search.as_deref() {
        jinfo!("Searching repositories with pattern: {}", search_pattern);

        let pattern = parse_search_pattern(search_pattern)?;
        let repositories = search_repositories(&client, &pattern, cli.num).await?;

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
        let releases = get_release_info(&client, repo, None).await?;

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
        let repo = cli
            .repo
            .as_deref()
            .ok_or_else(|| "--repo is required for info mode".to_string())?;
        let versions = info.split(',').collect::<Vec<&str>>();

        let releases = get_release_info(&client, repo, None).await?;

        for ver in versions {
            let release = releases
                .iter()
                .find(|r| r.tag_name.as_deref() == Some(ver))
                .ok_or_else(|| format!("Release with tag '{}' not found", ver))?;
            eprintln!("{}", release);
            eprintln!("---------------------");
        }
    } else {
        let repo = cli
            .repo
            .as_deref()
            .ok_or_else(|| "--repo is required for listing releases".to_string())?;
        let releases = get_release_info(&client, repo, Some(cli.num)).await?;
        eprintln!(
            "{:4} {:15} {:15} {:5} {:20} {:4}",
            "No", "Name", "Tag", "Type", "Published/Created", "Assets"
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

async fn search_repositories(
    client: &Client,
    pattern: &SearchPattern,
    num: usize,
) -> Result<Vec<Repository>> {
    match pattern {
        SearchPattern::UserAllRepos { username } => {
            // Use Search API to properly include private repos when authenticated
            let query = format!("user:{}", username);
            let url = format!(
                "https://api.github.com/search/repositories?q={}&per_page={}&page=1&sort=updated&order=desc",
                urlencoding::encode(&query),
                num
            );

            jdebug!("Searching user repos: {}", url);

            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to search repositories: {}", e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!(
                    "GitHub API request failed with status: {} (User '{}' may not exist)",
                    status, username
                ));
            }

            let search_response: SearchResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

            jinfo!(
                "Found {} repositories for user '{}'",
                search_response.total_count,
                username
            );
            Ok(search_response.items)
        }

        SearchPattern::UserWithKeyword { username, keyword } => {
            // Use Search API with user qualifier
            let query = format!("user:{} {}", username, keyword);
            let url = format!(
                "https://api.github.com/search/repositories?q={}&per_page={}&page=1&sort=stars&order=desc",
                urlencoding::encode(&query),
                num
            );

            jdebug!("Searching repositories: {}", url);

            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to search repositories: {}", e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!("GitHub API request failed with status: {}", status));
            }

            let search_response: SearchResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

            jinfo!(
                "Found {} repositories matching query",
                search_response.total_count
            );
            Ok(search_response.items)
        }

        SearchPattern::GlobalKeyword { keyword } => {
            // Use Search API for global search
            let url = format!(
                "https://api.github.com/search/repositories?q={}&per_page={}&page=1&sort=stars&order=desc",
                urlencoding::encode(keyword),
                num
            );

            jdebug!("Searching global repositories: {}", url);

            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to search repositories: {}", e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!("GitHub API request failed with status: {}", status));
            }

            let search_response: SearchResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

            jinfo!(
                "Found {} repositories matching keyword",
                search_response.total_count
            );
            Ok(search_response.items)
        }
    }
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

/// Validate that a repository exists and is accessible
async fn validate_repository(client: &Client, owner: &str, repo: &str) -> Result<RepositoryInfo> {
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
async fn validate_ref(client: &Client, owner: &str, repo: &str, ref_name: &str) -> Result<String> {
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

/// Check if git is installed and available in PATH
fn check_git_installed() -> Result<()> {
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
fn construct_clone_url(owner: &str, repo: &str, token: Option<&str>) -> String {
    if let Some(token) = token {
        format!("https://{}@github.com/{}/{}.git", token, owner, repo)
    } else {
        format!("https://github.com/{}/{}.git", owner, repo)
    }
}

/// Extract token from CLI arguments
fn extract_token_from_cli(cli: &Cli) -> Option<String> {
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

/// Execute git clone command
fn execute_git_clone(clone_url: &str, target_dir: &str, ref_name: Option<&str>) -> Result<()> {
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
fn cleanup_partial_clone(dir: &str) {
    jinfo!("Attempting to cleanup partial clone at '{}'...", dir);
    if let Err(e) = std::fs::remove_dir_all(dir) {
        jwarn!("Failed to cleanup directory '{}': {}", dir, e);
        jwarn!("Please manually remove the directory if it exists.");
    } else {
        jinfo!("Cleanup successful.");
    }
}

#[derive(Debug)]
enum SearchPattern {
    UserWithKeyword { username: String, keyword: String },
    UserAllRepos { username: String },
    GlobalKeyword { keyword: String },
}

fn parse_search_pattern(pattern: &str) -> Result<SearchPattern> {
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

/// Parse clone URL and extract owner, repo, and optional ref
fn parse_clone_url(url: &str) -> Result<CloneSpec> {
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
fn get_repo_name(url: &str) -> String {
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
        let result = parse_search_pattern("/kubernetes");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::GlobalKeyword { keyword } => {
                assert_eq!(keyword, "kubernetes");
            }
            _ => panic!("Expected GlobalKeyword pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_no_slash_global() {
        let result = parse_search_pattern("docker");
        assert!(result.is_ok());
        match result.unwrap() {
            SearchPattern::GlobalKeyword { keyword } => {
                assert_eq!(keyword, "docker");
            }
            _ => panic!("Expected GlobalKeyword pattern"),
        }
    }

    #[test]
    fn test_parse_search_pattern_empty_string() {
        let result = parse_search_pattern("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Search pattern cannot be empty");
    }

    #[test]
    fn test_parse_search_pattern_whitespace_only() {
        let result = parse_search_pattern("   ");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Search pattern cannot be empty");
    }

    #[test]
    fn test_parse_search_pattern_empty_global_keyword() {
        let result = parse_search_pattern("/");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Keyword cannot be empty for global search"
        );
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

    // Tests for Repository::summary() method
    // Note: summary() returns format: "{stars} {privacy} {full_name}"
    // Privacy indicator: "*" for private, " " for public

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
        // Public repos have a space, not "*"
        assert!(!summary.contains("*user"));
    }

    #[test]
    fn test_repository_summary_private_repo() {
        let repo = Repository {
            name: "private-repo".to_string(),
            full_name: "user/private-repo".to_string(),
            description: Some("A private repository".to_string()),
            stargazers_count: 100,
            html_url: "https://github.com/user/private-repo".to_string(),
            owner: Owner {
                login: "user".to_string(),
            },
            private: true,
        };

        let summary = repo.summary();
        // Private repos should have "*" indicator
        assert!(summary.contains("*"));
        assert!(summary.contains("user/private-repo"));
        assert!(summary.contains("100"));
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
}
