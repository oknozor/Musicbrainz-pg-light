use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub musicbrainz: MusicBrainzSettings,
    pub tables: TableSettings,
    pub schema: SchemaSettings,
}

#[derive(Debug, Deserialize, Default)]
pub struct DatabaseSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct MusicBrainzSettings {
    pub base_url: String,
    pub token: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct TableSettings {
    pub ignore: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct SchemaSettings {
    pub ignore: Vec<String>,
}

impl Settings {
    pub fn get() -> Result<Self, config::ConfigError> {
        let mut config = Config::builder().add_source(
            Environment::with_prefix("MBLIGHT")
                .try_parsing(true)
                .prefix_separator("__")
                .separator("__"),
        );

        let etc_config = PathBuf::from("/etc/mblight/config.toml");
        if etc_config.exists() {
            config = config.add_source(File::from(etc_config));
        }

        let default_config = PathBuf::from("config.toml");
        if default_config.exists() {
            config = config.add_source(File::from(default_config));
        }

        config.build()?.try_deserialize()
    }
}
