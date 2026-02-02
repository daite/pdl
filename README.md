# pdl (Podcast Downloader)

[![CI](https://github.com/daite/pdl/actions/workflows/ci.yml/badge.svg)](https://github.com/daite/pdl/actions/workflows/ci.yml)

A command-line application in Rust that fetches podcast episodes from multiple RSS feeds, allows interactive feed and episode selection, and downloads audio files with a fancy progress bar.

## Features

- Support for multiple podcast feeds
- Interactive feed selection
- Fetches podcast episodes from RSS feeds
- Interactive episode selection with arrow keys
- Beautiful ASCII art banner
- Real-time download progress bar with:
  - Elapsed time
  - Progress percentage
  - Download speed
  - ETA (estimated time remaining)
- Automatic filename sanitization
- Downloads saved to `podcast-downloads/` directory

## Installation

### Using Make

```bash
make install
```

### Build from source

```bash
cargo build --release
```

The compiled binary will be at `target/release/pdl`.

## Usage

### Default (show 10 episodes)
```bash
pdl
```

### Limit number of episodes displayed
```bash
pdl -n 5
```

### Show version
```bash
pdl -v
```

### Show help
```bash
pdl --help
```

## How it works

1. Application displays a banner
2. User selects a podcast feed from available options
3. Fetches RSS feed from the selected URL
4. Lists available episodes (limited by `-n` flag)
5. User selects an episode using arrow keys
6. Episode downloads with progress bar
7. Audio file saved to `podcast-downloads/` directory

## Configuration

The RSS feeds are configured in the `FEEDS` array in `src/main.rs`:
```rust
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
```

To add or modify feeds, edit this array in `src/main.rs` and rebuild.

## Dependencies

- `clap` - CLI argument parsing
- `rss` - RSS feed parsing
- `reqwest` - HTTP client (blocking mode)
- `indicatif` - Progress bar
- `inquire` - Interactive prompts
- `anyhow` - Error handling

## License

MIT
