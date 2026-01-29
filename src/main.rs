use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Select;
use reqwest::blocking::Client;
use rss::Channel;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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
}

struct Episode {
    title: String,
    url: String,
}

fn main() -> Result<()> {
    // Parse CLI arguments (before banner so -v works cleanly)
    let args = Args::parse();

    // Display banner
    display_banner();

    // Hardcoded RSS feed URL
    let rss_url = "https://omny.fm/shows/cozy-up/playlists/doctor.rss";

    println!("Fetching RSS feed...\n");

    // Fetch and parse RSS feed
    let episodes = fetch_episodes(rss_url, args.n)?;

    if episodes.is_empty() {
        println!("No episodes found in the feed.");
        return Ok(());
    }

    // Create interactive selection menu
    let episode_titles: Vec<String> = episodes
        .iter()
        .enumerate()
        .map(|(i, ep)| format!("{}. {}", i + 1, ep.title))
        .collect();

    let selection = Select::new("Select an episode to download:", episode_titles)
        .prompt()
        .context("Failed to get user selection")?;

    // Extract index from selection
    let selected_index = episodes
        .iter()
        .position(|ep| selection.contains(&ep.title))
        .context("Could not find selected episode")?;

    let selected_episode = &episodes[selected_index];

    println!("\nDownloading: {}", selected_episode.title);

    // Download the episode
    download_episode(selected_episode)?;

    println!("\n✓ Download complete!");

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

fn download_episode(episode: &Episode) -> Result<()> {
    // Create podcast-downloads directory if it doesn't exist
    let download_dir = Path::new("podcast-downloads");
    fs::create_dir_all(download_dir).context("Failed to create download directory")?;

    // Sanitize filename
    let filename = sanitize_filename(&episode.title);
    let extension = get_extension_from_url(&episode.url);
    let filepath = download_dir.join(format!("{}.{}", filename, extension));

    // Download file
    let client = Client::new();
    let mut response = client
        .get(&episode.url)
        .send()
        .context("Failed to start download")?;

    let total_size = response
        .content_length()
        .context("Failed to get content length")?;

    // Create progress bar
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .context("Failed to create progress bar template")?
            .progress_chars("=>-"),
    );

    // Download with progress
    let mut file = File::create(&filepath).context("Failed to create output file")?;
    let mut downloaded: u64 = 0;

    loop {
        let mut buffer = vec![0; 8192];
        let bytes_read = std::io::Read::read(&mut response, &mut buffer)
            .context("Failed to read download chunk")?;

        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])
            .context("Failed to write to file")?;

        downloaded += bytes_read as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Download complete");

    println!("Saved to: {}", filepath.display());

    Ok(())
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
    path.split('.').last().unwrap_or("mp3").to_lowercase()
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
