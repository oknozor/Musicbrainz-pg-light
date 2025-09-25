use crate::MbLight;
use crate::error::MbLightResult;
use crate::settings::MbLightSettingsExt;
use std::io::Read;
use std::path::Path;

use bytes::Bytes;
use indicatif::ProgressBar;
use sqlx::postgres::PgPoolCopyExt;
use std::fs;
use tar::Entry;
use tracing::info;

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn pg_copy(
        &self,
        mut entry: Entry<'_, impl Read>,
        schema: &str,
        table: &str,
        pb: ProgressBar,
    ) -> MbLightResult<()> {
        sqlx::query(&format!("ALTER TABLE {}.{} SET UNLOGGED", schema, table))
            .execute(&self.db)
            .await?;

        let tx = self.db.begin().await?;

        let mut sink = self
            .db
            .copy_in_raw(&format!("COPY {}.{} FROM STDIN", schema, table))
            .await?;

        let mut buffer = vec![0u8; 8 * 1024 * 1024];

        loop {
            let n = entry.read(&mut buffer)?;
            if n == 0 {
                break;
            }

            let chunk = Bytes::copy_from_slice(&buffer[..n]);
            sink.send(chunk).await?;

            pb.inc(n as u64);
        }

        sink.finish().await?;

        pb.set_message(format!("Committing on {schema}.{table}"));
        tx.commit().await?;
        sqlx::query(&format!("ALTER TABLE {}.{} SET LOGGED", schema, table))
            .execute(&self.db)
            .await?;

        pb.finish_with_message(format!("{schema}.{table} COPY done!"));
        Ok(())
    }

    pub async fn run_sql_file<P: AsRef<Path>>(&self, path: P) -> MbLightResult<()> {
        info!("Executing SQL file: {}", path.as_ref().display());
        let sql = fs::read_to_string(path)?;
        let sql = sql
            .lines()
            .filter(|line| !line.trim_start().starts_with('\\'))
            .collect::<Vec<_>>()
            .join("\n");
        sqlx::query("SET search_path TO musicbrainz, public")
            .execute(&self.db)
            .await?;
        sqlx::raw_sql(&sql).execute(&self.db).await?;

        Ok(())
    }

    pub async fn should_skip_table(&self, schema: &str, table: &str) -> MbLightResult<bool> {
        if self.config.should_skip_schema(schema) {
            return Ok(true);
        }

        if self.config.should_skip_table(table) {
            return Ok(true);
        }
        let fulltable = format!("{}.{}", schema, table);

        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                     SELECT FROM information_schema.tables
                     WHERE table_schema = $1 AND table_name = $2
                 )",
        )
        .bind(schema)
        .bind(table)
        .fetch_one(&self.db)
        .await
        .map(Option::unwrap_or_default)?;

        if !table_exists {
            info!("Skipping {} (table {} does not exist)", table, fulltable);
            return Ok(true);
        }

        let has_data: bool = self.has_data(schema, table).await?;

        if has_data {
            info!(
                "Skipping {} (table {} already contains data)",
                table, fulltable
            );
            return Ok(true);
        }

        Ok(false)
    }
}
