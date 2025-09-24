# MusicBrainz Light

A high-performance Rust implementation for creating and maintaining MusicBrainz database mirrors with automatic schema updates.

This project is a modernized clone of [mbslave](https://github.com/acoustid/mbslave) that provides significant performance improvements and automatic schema update capabilities, eliminating the need for manual schema migrations.

## Features

- 🚀 **Performance-Focused**: Written in Rust with async/await for improved performance
- 🔄 **Automatic Schema Updates**: Handles schema changes automatically without manual intervention
- 📦 **Straightforward Setup**: Simple configuration and Docker support
- 🎯 **Selective Sync**: Configure which schemas and tables to replicate
- 📊 **Progress Tracking**: Built-in progress bars for long-running operations
- 🔧 **Flexible Configuration**: Support for TOML files and environment variables
- 🐳 **Docker Ready**: Includes Docker Compose setup for deployment

## Improvements over mbslave

- **Automatic Schema Updates**: No more manual schema upgrade scripts
- **Better Performance**: Rust implementation with optimized database operations
- **Modern Tooling**: Built with modern async Rust ecosystem
- **Simplified Maintenance**: Reduced operational overhead
- **Enhanced Error Handling**: Better error messages and recovery

## Installation

### From Source

```bash
git clone https://github.com/your-org/musicbrainz-light
cd musicbrainz-light
cargo install --path .
```

The binary will be available at `target/release/mbpg-light`.

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
mbpg-light sync
```

This command will:
1. Download and apply replication packets
2. Automatically handle schema updates
3. Process pending data changes
4. Continue until all updates are applied

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
- `notify`: Enable reindex notifications

```bash
# Build without CLI features
cargo build --no-default-features

# Build with specific features
cargo build --features "progress,notify"
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
