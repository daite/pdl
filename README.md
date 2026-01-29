# pdl (Podcast Downloader)

A command-line application in Rust that fetches podcast episodes from an RSS feed, allows interactive episode selection, and downloads audio files with a fancy progress bar.

## Features

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
2. Fetches RSS feed from the configured URL
3. Lists available episodes (limited by `-n` flag)
4. User selects an episode using arrow keys
5. Episode downloads with progress bar
6. Audio file saved to `podcast-downloads/` directory

## Configuration

The RSS feed URL is currently hardcoded in the source:
```rust
let rss_url = "https://omny.fm/shows/cozy-up/playlists/doctor.rss";
```

To change the feed, modify this line in `src/main.rs` and rebuild.

## Dependencies

- `clap` - CLI argument parsing
- `rss` - RSS feed parsing
- `reqwest` - HTTP client (blocking mode)
- `indicatif` - Progress bar
- `inquire` - Interactive prompts
- `anyhow` - Error handling

## License

MIT
