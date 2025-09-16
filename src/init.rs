use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::MbLight;

impl MbLight {
    pub async fn create_schemas(&self) -> Result<()> {
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
            if self.config.schema.should_skip(&schema) {
                continue;
            }

            let query = format!("CREATE SCHEMA IF NOT EXISTS {}", schema);
            println!("Executing query: {}", query);
            self.db.execute(&query, &[]).await?;
        }

        Ok(())
    }

    async fn run_sql_file(&self, path: &str) -> Result<()> {
        println!("Executing SQL file: {}", path);
        let sql = fs::read_to_string(path)?;
        let sql = sql
            .lines()
            .filter(|line| !line.trim_start().starts_with('\\'))
            .collect::<Vec<_>>()
            .join("\n");

        self.db.batch_execute(&sql).await?;

        self.db
            .execute("SET search_path TO musicbrainz, public", &[])
            .await?;

        Ok(())
    }

    pub async fn run_all_scripts(&self, local_path: PathBuf) -> Result<()> {
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
            if self.config.schema.should_skip(schema) {
                continue;
            }
            let path = local_path.join(sql_script);
            self.run_sql_file(path.to_str().unwrap()).await?;
        }

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
            if self.config.schema.should_skip(&schema) {
                continue;
            }
            let path = local_path.join(sql_script);
            self.run_sql_file(path.to_str().unwrap()).await?;
        }

        Ok(())
    }
}
