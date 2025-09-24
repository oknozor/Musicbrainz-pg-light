use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::Deserialize;

use crate::error::MbLightResult;

pub trait MbLightSettingsExt {
    fn db_user(&self) -> &str;
    fn db_password(&self) -> &str;
    fn db_host(&self) -> &str;
    fn db_port(&self) -> u16;
    fn db_name(&self) -> &str;
    fn table_keep_only(&self) -> &Vec<String>;
    fn schema_keep_only(&self) -> &Vec<String>;
    fn musicbrainz_url(&self) -> &str;
    fn musicbrainz_token(&self) -> &str;
    fn should_skip_table(&self, table: &str) -> bool;
    fn should_skip_schema(&self, schema: &str) -> bool;
}

impl MbLightSettingsExt for Settings {
    fn db_user(&self) -> &str {
        &self.db.user
    }

    fn db_password(&self) -> &str {
        &self.db.password
    }

    fn db_host(&self) -> &str {
        &self.db.host
    }

    fn db_port(&self) -> u16 {
        self.db.port
    }

    fn db_name(&self) -> &str {
        &self.db.name
    }

    fn table_keep_only(&self) -> &Vec<String> {
        &self.tables.keep_only
    }

    fn schema_keep_only(&self) -> &Vec<String> {
        &self.schema.keep_only
    }

    fn musicbrainz_url(&self) -> &str {
        &self.musicbrainz.url
    }

    fn musicbrainz_token(&self) -> &str {
        &self.musicbrainz.token
    }

    fn should_skip_table(&self, table: &str) -> bool {
        let keep = self.table_keep_only();
        !keep.is_empty() && keep.iter().find(|t| t == &table).is_none()
    }

    fn should_skip_schema(&self, schema: &str) -> bool {
        let keep = self.schema_keep_only();
        !keep.is_empty() && keep.iter().find(|s| s == &schema).is_none()
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Settings {
    pub db: DBSettings,
    pub musicbrainz: MusicbrainzSettings,
    pub tables: TableSettings,
    pub schema: SchemaSettings,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct DBSettings {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub name: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct MusicbrainzSettings {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct TableSettings {
    keep_only: Vec<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SchemaSettings {
    keep_only: Vec<String>,
}

impl Settings {
    pub fn get() -> MbLightResult<Self> {
        let mut config = Config::builder().add_source(
            Environment::with_prefix("metadada")
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

        config.build()?.try_deserialize().map_err(Into::into)
    }

    pub fn db_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.db.user, self.db.password, self.db.host, self.db.port, self.db.name
        )
    }
}
