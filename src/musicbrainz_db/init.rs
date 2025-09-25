use std::path::Path;

use crate::error::MbLightResult;
use crate::progress::get_progress_bar;
use crate::settings::MbLightSettingsExt;
use crate::{MbLight, download::musicbrainz::MUSICBRAINZ_FTP, tar_helper::get_archive};
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tracing::{error, info};

const MB_DUMP: &str = "mbdump.tar.bz2";
const MB_DUMP_DERIVED: &str = "mbdump-derived.tar.bz2";
const COVER_ART_ARCHIVE: &str = "mbdump-cover-art-archive.tar.bz2";
const EVENT_ART_ARCHIVE: &str = "mbdump-even-art-archive.tar.bz2";
const MB_DUMP_STATS: &str = "mbdump-stats.tar.bz2";

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn create_schemas(&mut self) -> MbLightResult<()> {
        let schemas = [
            "musicbrainz",
            "cover_art_archive",
            "event_art_archive",
            "statistics",
            "documentation",
            "wikidocs",
            "dbmirror2",
        ];

        for schema in schemas {
            if self.config.should_skip_schema(schema) {
                continue;
            }

            let query = format!("CREATE SCHEMA IF NOT EXISTS {}", schema);
            info!("Executing query: {}", query);
            sqlx::query(&query).execute(&self.db).await?;
            self.alter_search_path().await?;
            self.reconnect().await?;
        }

        Ok(())
    }

    pub async fn run_all_scripts(&mut self, local_path: PathBuf) -> MbLightResult<()> {
        let sql_scripts = vec![
            ("musicbrainz", "CreatePrimaryKeys.sql"),
            ("cover_art_archive", "caa/CreatePrimaryKeys.sql"),
            ("event_art_archive", "eaa/CreatePrimaryKeys.sql"),
            ("statistics", "statistics/CreatePrimaryKeys.sql"),
            ("documentation", "documentation/CreatePrimaryKeys.sql"),
            ("wikidocs", "wikidocs/CreatePrimaryKeys.sql"),
            ("musicbrainz", "CreateFunctions.sql"),
            ("musicbrainz", "CreateMirrorOnlyFunctions.sql"),
            ("cover_art_archive", "caa/CreateFunctions.sql"),
            ("event_art_archive", "eaa/CreateFunctions.sql"),
            ("musicbrainz", "CreateIndexes.sql"),
            ("musicbrainz", "CreateMirrorIndexes.sql"),
            ("cover_art_archive", "caa/CreateIndexes.sql"),
            ("event_art_archive", "eaa/CreateIndexes.sql"),
            ("statistics", "statistics/CreateIndexes.sql"),
            ("musicbrainz", "CreateViews.sql"),
            ("cover_art_archive", "caa/CreateViews.sql"),
            ("event_art_archive", "eaa/CreateViews.sql"),
            ("musicbrainz", "CreateMirrorOnlyTriggers.sql"),
            ("musicbrainz", "ReplicationSetup.sql"),
            ("dbmirror2", "dbmirror2/ReplicationSetup.sql"),
        ];

        for (schema, sql_script) in sql_scripts {
            if self.config.should_skip_schema(schema) {
                continue;
            }
            let path = local_path.join(sql_script);
            self.run_sql_file(path.to_str().unwrap()).await?;
        }

        Ok(())
    }

    pub async fn create_tables(&mut self, local_path: &Path) -> MbLightResult<()> {
        self.run_sql_file(local_path.join("Extensions.sql").to_str().unwrap())
            .await?;
        self.run_sql_file(
            local_path
                .join("CreateSearchConfiguration.sql")
                .to_str()
                .unwrap(),
        )
        .await?;
        let sql_scripts = vec![
            // types
            ("musicbrainz", "CreateCollations.sql"),
            ("musicbrainz", "CreateTypes.sql"),
            // tables
            ("musicbrainz", "CreateTables.sql"),
            ("cover_art_archive", "caa/CreateTables.sql"),
            ("event_art_archive", "eaa/CreateTables.sql"),
            ("statistics", "statistics/CreateTables.sql"),
            ("documentation", "documentation/CreateTables.sql"),
            ("wikidocs", "wikidocs/CreateTables.sql"),
        ];
        for (schema, sql_script) in sql_scripts {
            if self.config.should_skip_schema(schema) {
                continue;
            }
            let path = local_path.join(sql_script);
            self.run_sql_file(path.to_str().unwrap()).await?;
        }
        Ok(())
    }

    async fn alter_search_path(&mut self) -> MbLightResult<()> {
        let username = self.config.db_user();
        sqlx::query(&format!(
            "ALTER USER {username} SET search_path = musicbrainz, public;"
        ))
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn ingest_dump(&mut self) -> MbLightResult<()> {
        let mut filenames = vec![MB_DUMP, MB_DUMP_DERIVED];

        if !self.config.should_skip_schema("statistics") {
            filenames.push(MB_DUMP_STATS);
        }
        if !self.config.should_skip_schema("cover_art_archive") {
            filenames.push(COVER_ART_ARCHIVE);
        }
        if !self.config.should_skip_schema("event_art_archive") {
            filenames.push(EVENT_ART_ARCHIVE);
        }

        let latest = self.get_latest().await?;
        info!("Latest version: {}", latest);

        for filename in filenames {
            let url = format!("{}/{}/{}", MUSICBRAINZ_FTP, latest, filename);
            let tempfile = NamedTempFile::new()?;
            let mut writer = tempfile.reopen()?;
            self.download_with_progress(&url, &mut writer).await?;
            let mut archive = get_archive(tempfile.path())?;

            info!("Starting pg_copy for {filename}");

            for entry in archive.entries()? {
                match entry {
                    Ok(entry) => {
                        let path = entry.path()?;
                        let entry_size = entry.header().entry_size()?;
                        let name = path.to_string_lossy().into_owned();

                        if !name.starts_with("mbdump/") {
                            continue;
                        }

                        let filename = name.strip_prefix("mbdump/").unwrap();
                        let filename = filename.strip_suffix("_sanitised").unwrap_or(filename);

                        let (schema, table) = filename
                            .split_once('.')
                            .unwrap_or(("musicbrainz", filename));

                        if self.should_skip_table(schema, table).await? {
                            continue;
                        }

                        let pb = get_progress_bar(entry_size)?;
                        pb.set_message(table.to_string());

                        self.pg_copy(entry, schema, table, pb).await?;
                    }
                    Err(err) => {
                        error!("{err}");
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}
