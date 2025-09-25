<div align="center">

# MusicBrainz Light

[![Crates.io](https://img.shields.io/crates/v/musicbrainz-light)](https://crates.io/crates/musicbrainz-light)
[![GitHub tag (latest SemVer)](https://img.shields.io/github/v/tag/oknozor/mbpg-light)](https://github.com/oknozor/mbpg-light/tags)
[![CI](https://github.com/oknozor/mbpg-light/actions/workflows/CI.yaml/badge.svg)](https://github.com/oknozor/mbpg-light/actions/workflows/CI.yaml)

</div>

A high-performance Rust implementation for creating and maintaining MusicBrainz database mirrors with automatic schema updates.

This project is a modernized clone of [mbslave](https://github.com/acoustid/mbslave) that provides significant performance improvements and automatic schema update capabilities, eliminating the need for manual schema migrations.

## Features

- 🚀 **High Performance**: Written in Rust with async/await for optimal performance
- 🔄 **Automatic Schema Updates**: Handles schema changes automatically without manual intervention
- 📦 **Easy Setup**: Simple configuration and Docker support
- 🎯 **Selective Sync**: Configure which schemas and tables to replicate
- 📊 **Progress Tracking**: Built-in progress bars for long-running operations
- 🔧 **Flexible Configuration**: Support for TOML files and environment variables
- 🐳 **Docker Ready**: Includes Docker Compose setup for easy deployment

## Improvements over mbslave

- **Automatic Schema Updates**: No more manual schema upgrade scripts
- **Better Performance**: Rust implementation with optimized database operations
- **Modern Tooling**: Built with modern async Rust ecosystem
- **Simplified Maintenance**: Reduced operational overhead
- **Enhanced Error Handling**: Better error messages and recovery

## Installation

### From Source

```bash
git clone https://github.com/oknozor/musicbrainz-light
cd musicbrainz-light
cargo install --path .
```

The binary will be available at `target/release/mbpg-light`.

### Using cargo

```bash
cargo install musicbrainz-light
```

## Configuration

### Configuration File

Create a `config.toml` file in your project directory or `/etc/mblight/config.toml`:

```toml
[db]
user = "musicbrainz"
password = "musicbrainz"
host = "localhost"
port = 5432
name = "musicbrainz"

[musicbrainz]
url = "https://data.musicbrainz.org"
token = "your-musicbrainz-token"

[tables]
# Optional: specify which tables to keep (empty = keep all)
keep_only = []

[schema]
# Optional: specify which schemas to keep (empty = keep all)
keep_only = []
```

### Getting a MusicBrainz Token

1. Visit [MetaBrainz website](https://metabrainz.org/)
2. Create an account or log in
3. Generate an API token for database replication

## Usage

### Initialize Database

To create a new MusicBrainz mirror from scratch:

```bash
mbpg-light init
```

This command will:
1. Download the latest MusicBrainz SQL dump
2. Create database schemas
3. Import all data
4. Set up replication control
5. Apply indexes and constraints

### Sync Database

To keep your database up-to-date with incremental changes:

```bash
# Sync once and exit when caught up
mbpg-light sync

# Sync continuously, waiting for new replication packets
mbpg-light sync --loop
```

This command will:
1. Download and apply replication packets
2. Automatically handle schema updates
3. Process pending data changes
4. Continue until all updates are applied (or loop infinitely with `--loop`)

## Logging

Configure logging levels using the `RUST_LOG` environment variable:

```bash
# Info level (default)
export RUST_LOG=mbpg_light=info,musicbrainz_light=info

# Debug level
export RUST_LOG=mbpg_light=debug,musicbrainz_light=debug
```

## Selective Replication

You can configure which schemas and tables to replicate by modifying your `config.toml`:

```toml
[schema]
keep_only = ["musicbrainz", "cover_art_archive"]

[tables]
keep_only = ["artist", "release", "recording", "work"]
```

This is useful for:
- Reducing database size
- Focusing on specific data subsets
- Testing with smaller datasets

## Using as a library

You can use `musicbrainz-light` as a library in your Rust projects for programmatic access to MusicBrainz database operations.

### Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
musicbrainz-light = "0.1"
tokio = { version = "1", features = ["rt-multi-thread"] }
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
```

### Feature Flags

The library supports several optional features:

- `progress` (default): Enables progress bars during operations
- `cli`: Command-line interface dependencies (not needed for library usage)

```toml
# Minimal library usage without CLI dependencies
[dependencies]
musicbrainz-light = { version = "0.1", default-features = false }

# With progress bars
[dependencies]
musicbrainz-light = { version = "0.1", default-features = false, features = ["progress"] }
```

### Basic Usage

```rust
use musicbrainz_light::{MbLight, settings::Settings, MbLightError};

#[tokio::main]
async fn main() -> Result<(), MbLightError> {
    // Load configuration
    let config = Settings::get()?;

    // Create MbLight instance
    let mut mb_light = MbLight::try_new(config, config.db_url()).await?;

    // Initialize database (first time setup)
    mb_light.init().await?;

    // Or sync with existing database (sync once)
    mb_light.sync(false).await?;

    // Or sync infinitely
    mb_light.sync(true).await?;

    Ok(())
}
```

### Configuration Options

#### Using Default Settings

The default `Settings` struct loads configuration from:
1. Environment variables with `METADADA__` prefix
2. `/etc/mblight/config.toml`
3. `config.toml` in current directory

```rust
use musicbrainz_light::settings::Settings;

let settings = Settings::get()?;
```

#### Custom Settings Implementation

You can implement your own configuration by implementing the `MbLightSettingsExt` trait:

```rust
use musicbrainz_light::settings::MbLightSettingsExt;

struct MyCustomSettings {
    db_host: String,
    db_user: String,
    // ... other fields
}

impl MbLightSettingsExt for MyCustomSettings {
    fn db_user(&self) -> &str { &self.db_user }
    fn db_password(&self) -> &str { "my_password" }
    fn db_host(&self) -> &str { &self.db_host }
    fn db_port(&self) -> u16 { 5432 }
    fn db_name(&self) -> &str { "musicbrainz" }
    fn table_keep_only(&self) -> &Vec<String> { &vec![] }
    fn schema_keep_only(&self) -> &Vec<String> { &vec![] }
    fn musicbrainz_url(&self) -> &str { "https://data.musicbrainz.org" }
    fn musicbrainz_token(&self) -> &str { "your_token" }
    fn should_skip_table(&self, _table: &str) -> bool { false }
    fn should_skip_schema(&self, _schema: &str) -> bool { false }
}

// Use with MbLight
let custom_config = MyCustomSettings { /* ... */ };
let mb_light = MbLight::try_new(custom_config, db_url).await?;
```

### With Notifications

When building `MbLight` with a `mpsc::Sender` , you can receive notifications when replication reaches the latest packet:

```rust
use tokio::sync::mpsc;

async fn with_notifications() -> Result<(), MbLightError> {
    let config = Settings::get()?;

    // Create notification channel
    let (tx, mut rx) = mpsc::channel(10);

    // Create MbLight instance and add notification sender
    let mb_light = MbLight::try_new(config, config.db_url()).await?.with_sender(tx);

    // Spawn sync task
    let sync_handle = tokio::spawn(async move {
        mb_light.sync(true).await  // Use infinite sync
    });

    // Listen for reindex notifications
    tokio::spawn(async move {
        while let Some(_) = rx.recv().await {
            println!("Replication caught up - time to reindex!");
            // Trigger your reindexing logic here
        }
    });

    sync_handle.await??;
    Ok(())
}
```

### Utility Methods

Check if a table has data:

```rust
let has_artists = mb_light.has_data("musicbrainz", "artist").await?;
if has_artists {
    println!("Artist table contains data");
}
```

## Development

### Prerequisites

- Rust 1.75+ (2024 edition)
- PostgreSQL 12+
- Git

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings
```

### Features

The project supports several feature flags:

- `cli` (default): Command-line interface with colored output
- `progress` (default): Progress bars for long operations

```bash
# Build without CLI features
cargo build --no-default-features

# Build with specific features
cargo build --features "progress"
```

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Original [mbslave](https://github.com/acoustid/mbslave) project by Lukáš Lalinský
- [MusicBrainz](https://musicbrainz.org/) for providing the database and replication infrastructure
- The Rust community for excellent async and database libraries

## Support

- Open an issue for bug reports or feature requests
- Check existing issues before creating new ones
- Provide detailed information including configuration and logs

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and changes.
