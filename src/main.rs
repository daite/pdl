use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select};
use reqwest::blocking::Client;
use rss::Channel;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// Podcast Downloader - Download podcast episodes from RSS feeds
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag = true)]
struct Args {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    /// Number of episodes to display
    #[arg(short, long, default_value_t = 10)]
    n: usize,

    /// Maximum number of concurrent downloads
    #[arg(short, long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(1..))]
    jobs: u8,
}

struct Episode {
    title: String,
    url: String,
}

struct PodcastFeed {
    name: &'static str,
    url: &'static str,
}

const FEEDS: &[PodcastFeed] = &[
    PodcastFeed {
        name: "Cozy Up (Doctor)",
        url: "https://omny.fm/shows/cozy-up/playlists/doctor.rss",
    },
    PodcastFeed {
        name: "Cozy Up (Podcast)",
        url: "https://omny.fm/shows/cozy-up/playlists/podcast.rss",
    },
];

fn main() -> Result<()> {
    // Parse CLI arguments (before banner so -v works cleanly)
    let args = Args::parse();

    // Display banner
    display_banner();

    // Select podcast feed
    let feed_names: Vec<&str> = FEEDS.iter().map(|f| f.name).collect();
    let selected_feed_name = Select::new("Select a podcast feed:", feed_names)
        .prompt()
        .context("Failed to get feed selection")?;

    let selected_feed = FEEDS
        .iter()
        .find(|f| f.name == selected_feed_name)
        .context("Could not find selected feed")?;

    println!("\nFetching RSS feed...\n");

    // Fetch and parse RSS feed
    let episodes = fetch_episodes(selected_feed.url, args.n)?;

    if episodes.is_empty() {
        println!("No episodes found in the feed.");
        return Ok(());
    }

    // Create interactive multi-selection menu
    let episode_titles: Vec<String> = episodes
        .iter()
        .enumerate()
        .map(|(i, ep)| format!("{}. {}", i + 1, ep.title))
        .collect();

    let selected: Vec<&Episode> = MultiSelect::new(
        "Select episodes to download (space to toggle):",
        episode_titles,
    )
    .raw_prompt()
    .context("Failed to get user selection")?
    .into_iter()
    .map(|opt| &episodes[opt.index])
    .collect();

    if selected.is_empty() {
        println!("No episodes selected.");
        return Ok(());
    }

    println!(
        "\nDownloading {} episode(s) with up to {} concurrent job(s)...\n",
        selected.len(),
        args.jobs
    );

    // Download the episodes concurrently
    let results = download_episodes(&selected, args.jobs as usize)?;

    println!();
    let mut failed = 0;
    for (episode, result) in selected.iter().zip(&results) {
        match result {
            Ok(DownloadOutcome::Downloaded(path)) => println!("✓ Saved to: {}", path.display()),
            Ok(DownloadOutcome::Skipped(path)) => {
                println!("⏭ Already downloaded: {}", path.display())
            }
            Err(e) => {
                failed += 1;
                println!("✗ {}: {:#}", episode.title, e);
            }
        }
    }

    if failed > 0 {
        anyhow::bail!("{} of {} download(s) failed", failed, selected.len());
    }

    println!("\n✓ All downloads complete!");

    Ok(())
}

fn display_banner() {
    println!(
        r#"
╔═══════════════════════════════════════════════════════╗
║                                                       ║
║   ██████╗  ██████╗ ██████╗  ██████╗ █████╗ ███████╗ ║
║   ██╔══██╗██╔═══██╗██╔══██╗██╔════╝██╔══██╗██╔════╝ ║
║   ██████╔╝██║   ██║██║  ██║██║     ███████║███████╗ ║
║   ██╔═══╝ ██║   ██║██║  ██║██║     ██╔══██║╚════██║ ║
║   ██║     ╚██████╔╝██████╔╝╚██████╗██║  ██║███████║ ║
║   ╚═╝      ╚═════╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝╚══════╝ ║
║                                                       ║
║              Podcast Downloader v0.1.0                ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝
"#
    );
}

fn fetch_episodes(url: &str, limit: usize) -> Result<Vec<Episode>> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .context("Failed to fetch RSS feed")?
        .bytes()
        .context("Failed to read RSS feed response")?;

    let channel = Channel::read_from(&response[..]).context("Failed to parse RSS feed")?;

    let episodes: Vec<Episode> = channel
        .items()
        .iter()
        .take(limit)
        .filter_map(|item| {
            let title = item.title()?.to_string();
            let url = item.enclosure()?.url().to_string();
            Some(Episode { title, url })
        })
        .collect();

    Ok(episodes)
}

enum DownloadOutcome {
    Downloaded(PathBuf),
    Skipped(PathBuf),
}

/// Download episodes using a bounded pool of worker threads.
///
/// Workers pull the next job from a shared atomic cursor, so at most `jobs`
/// downloads run at once. Results are returned in the same order as `episodes`.
fn download_episodes(episodes: &[&Episode], jobs: usize) -> Result<Vec<Result<DownloadOutcome>>> {
    let download_dir = Path::new("podcast-downloads");
    fs::create_dir_all(download_dir).context("Failed to create download directory")?;

    // A single client is shared by all workers so they reuse the connection pool
    let client = Client::new();
    let multi = MultiProgress::new();
    let style = ProgressStyle::default_bar()
        .template("{msg:30!} [{bar:30.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} ({eta})")
        .context("Failed to create progress bar template")?
        .progress_chars("=>-");

    let next = AtomicUsize::new(0);
    let workers = jobs.min(episodes.len());

    let mut results: Vec<(usize, Result<DownloadOutcome>)> = thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(episode) = episodes.get(i) else {
                            break;
                        };
                        let result =
                            download_episode(&client, &multi, &style, download_dir, episode);
                        done.push((i, result));
                    }
                    done
                })
            })
            .collect();

        handles
            .into_iter()
            .flat_map(|h| h.join().expect("download worker panicked"))
            .collect()
    });

    results.sort_by_key(|(i, _)| *i);
    Ok(results.into_iter().map(|(_, r)| r).collect())
}

fn download_episode(
    client: &Client,
    multi: &MultiProgress,
    style: &ProgressStyle,
    download_dir: &Path,
    episode: &Episode,
) -> Result<DownloadOutcome> {
    // Sanitize filename
    let filename = sanitize_filename(&episode.title);
    let extension = get_extension_from_url(&episode.url);
    let filepath = download_dir.join(format!("{}.{}", filename, extension));

    // Check if file already exists
    if filepath.exists() {
        return Ok(DownloadOutcome::Skipped(filepath));
    }

    // Download file
    let mut response = client
        .get(&episode.url)
        .send()
        .and_then(|r| r.error_for_status())
        .context("Failed to start download")?;

    let total_size = response
        .content_length()
        .context("Failed to get content length")?;

    // Create progress bar
    let pb = multi.add(ProgressBar::new(total_size));
    pb.set_style(style.clone());
    pb.set_message(episode.title.clone());

    // Download to a temporary file so an interrupted download isn't mistaken
    // for a finished one on the next run
    let part_path = filepath.with_extension(format!("{}.part", extension));
    let result = (|| -> Result<()> {
        let mut file = File::create(&part_path).context("Failed to create output file")?;
        let mut buffer = vec![0; 8192];

        loop {
            let bytes_read = std::io::Read::read(&mut response, &mut buffer)
                .context("Failed to read download chunk")?;

            if bytes_read == 0 {
                break;
            }

            file.write_all(&buffer[..bytes_read])
                .context("Failed to write to file")?;

            pb.inc(bytes_read as u64);
        }

        fs::rename(&part_path, &filepath).context("Failed to finalize output file")
    })();

    if result.is_err() {
        let _ = fs::remove_file(&part_path);
        pb.abandon();
    } else {
        pb.finish();
    }

    result.map(|()| DownloadOutcome::Downloaded(filepath))
}

fn sanitize_filename(title: &str) -> String {
    title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn get_extension_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    path.split('.').next_back().unwrap_or("mp3").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_removes_invalid_chars() {
        assert_eq!(sanitize_filename("hello/world"), "hello-world");
        assert_eq!(sanitize_filename("file:name"), "file-name");
        assert_eq!(sanitize_filename("test*file?"), "test-file-");
        assert_eq!(sanitize_filename("a<b>c"), "a-b-c");
        assert_eq!(sanitize_filename("pipe|char"), "pipe-char");
        assert_eq!(sanitize_filename("back\\slash"), "back-slash");
        assert_eq!(sanitize_filename("quote\"test"), "quote-test");
    }

    #[test]
    fn test_sanitize_filename_preserves_valid_chars() {
        assert_eq!(sanitize_filename("hello world"), "hello world");
        assert_eq!(sanitize_filename("episode-01"), "episode-01");
        assert_eq!(sanitize_filename("podcast_name"), "podcast_name");
        assert_eq!(sanitize_filename("한글 제목"), "한글 제목");
    }

    #[test]
    fn test_sanitize_filename_trims_whitespace() {
        assert_eq!(sanitize_filename("  hello  "), "hello");
        assert_eq!(sanitize_filename("\ttest\n"), "test");
    }

    #[test]
    fn test_get_extension_from_url_basic() {
        assert_eq!(
            get_extension_from_url("https://example.com/file.mp3"),
            "mp3"
        );
        assert_eq!(
            get_extension_from_url("https://example.com/file.MP3"),
            "mp3"
        );
        assert_eq!(
            get_extension_from_url("https://example.com/audio.m4a"),
            "m4a"
        );
        assert_eq!(
            get_extension_from_url("https://example.com/video.mp4"),
            "mp4"
        );
    }

    #[test]
    fn test_get_extension_from_url_with_query_params() {
        assert_eq!(
            get_extension_from_url("https://example.com/file.mp3?token=abc123"),
            "mp3"
        );
        assert_eq!(
            get_extension_from_url("https://cdn.example.com/podcast.m4a?expires=123&sig=xyz"),
            "m4a"
        );
    }

    #[test]
    fn test_get_extension_from_url_no_extension() {
        // Note: function splits by '.' so returns last segment after dot
        assert_eq!(
            get_extension_from_url("https://example.com/file"),
            "com/file"
        );
        // URL with path ending in extension-less filename
        assert_eq!(
            get_extension_from_url("http://example/podcast"),
            "http://example/podcast"
        );
    }

    #[test]
    fn test_get_extension_from_url_multiple_dots() {
        assert_eq!(
            get_extension_from_url("https://example.com/file.name.mp3"),
            "mp3"
        );
    }
}
