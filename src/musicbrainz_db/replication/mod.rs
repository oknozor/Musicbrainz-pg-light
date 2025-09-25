use std::io::Read;

use crate::{
    MbLight,
    error::{MbLightError, MbLightResult},
    musicbrainz_db::replication::{
        pending_data::PendingData, replication_control::ReplicationControl,
    },
    progress::get_progress_bar,
    settings::MbLightSettingsExt,
    tar_helper::get_archive,
};
use itertools::Itertools;
use sqlx::types::chrono::{DateTime, Utc};
use tempfile::NamedTempFile;
use tracing::{debug, error, info};

mod pending_data;
pub(crate) mod replication_control;

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn apply_pending_replication(&self) -> Result<(), MbLightError> {
        let remains = PendingData::all(&self.db).await?;
        if !remains.is_empty() {
            let replication_control = ReplicationControl::get(&self.db).await?;
            info!("Applying unfinished replication packet");
            self.apply_pending_data().await?;
            info!("Replication finished");
            replication_control.update(&self.db).await?;
        }

        let replication_control = ReplicationControl::get(&self.db).await?;

        let next_replication_sequence = replication_control.next_replication_sequence()?;
        let last_replication_date = replication_control
            .last_replication_date
            .map(|d| d.format("%y/%m/%d - %H:%M:%S").to_string())
            .unwrap_or("N/a".into());
        info!(
            "Starting new replication process, last replication occured on {last_replication_date}",
        );
        let tmpfile = NamedTempFile::new()?;
        {
            let mut writer = tmpfile.reopen()?;
            let packet_url = replication_control.next_replication_packet_url(
                self.config.musicbrainz_url(),
                self.config.musicbrainz_token(),
            )?;

            self.download_with_progress(&packet_url, &mut writer)
                .await?;
        }

        info!(
            "Replication packet {} downloaded, processing...",
            next_replication_sequence
        );
        let mut archive = get_archive(tmpfile.path())?;

        for entry in archive.entries()? {
            self.process_replication_entry(&replication_control, entry, next_replication_sequence)
                .await?;
        }

        self.apply_pending_data().await?;
        info!("replication finished");
        replication_control.update(&self.db).await?;

        Ok(())
    }

    pub async fn drop_tablecheck(&self) -> MbLightResult<()> {
        sqlx::query(
            "ALTER TABLE dbmirror2.pending_data DROP CONSTRAINT IF EXISTS tablename_exists;",
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn process_replication_entry(
        &self,
        replication_control: &ReplicationControl,
        entry: Result<tar::Entry<'_, impl Read>, std::io::Error>,
        next_replication_sequence: i32,
    ) -> Result<(), MbLightError> {
        match entry {
            Ok(mut entry) => {
                let path = entry.path()?;
                let filename = path.as_ref().file_name().and_then(|f| f.to_str());
                debug!("processing {}", filename.unwrap_or("unknown"));
                match filename {
                    Some("pending_data") => {
                        let pb = get_progress_bar(entry.size())?;
                        self.pg_copy(entry, "dbmirror2", "pending_data", pb).await?;
                    }
                    Some("pending_keys") => {
                        let pb = get_progress_bar(entry.size())?;
                        self.pg_copy(entry, "dbmirror2", "pending_keys", pb).await?;
                    }
                    Some("REPLICATION_SEQUENCE") => {
                        let mut replication_sequence = String::new();
                        let _ = entry.read_to_string(&mut replication_sequence);
                        let replication_sequence = replication_sequence.trim();
                        let replication_sequence = replication_sequence.parse::<i32>()?;
                        if replication_sequence != next_replication_sequence {
                            return Err(MbLightError::SequenceMissmatch {
                                expected: next_replication_sequence,
                                got: replication_sequence,
                            });
                        }
                    }
                    Some("SCHEMA_SEQUENCE") => {
                        let mut schema_sequence = String::new();
                        let _ = entry.read_to_string(&mut schema_sequence)?;
                        let schema_sequence = schema_sequence.trim();
                        let schema_sequence = schema_sequence.parse::<i32>()?;
                        if replication_control.is_next(schema_sequence)? {
                            info!("Downloading schema update");
                            let path = self.download_schema_update(schema_sequence).await?;
                            info!("Updating schema to version {}", schema_sequence);
                            self.run_sql_file(path).await?;
                        } else if !replication_control.schema_sequence_match(schema_sequence) {
                            return Err(MbLightError::SchemaMissmatch {
                                expected: replication_control
                                    .current_replication_sequence
                                    .unwrap_or_default(),
                                got: schema_sequence,
                            });
                        }
                    }
                    Some("TIMESTAMP") => {
                        extract_timestamp(entry)?;
                    }
                    _ => {}
                }
            }
            Err(err) => {
                error!("Error skipping archive entry: {err}");
            }
        };

        Ok(())
    }

    async fn apply_pending_data(&self) -> MbLightResult<()> {
        let mut pending_data = PendingData::all(&self.db).await?;
        pending_data.retain(|p| {
            let (schema, table) = p.split_table_schema();
            !self.config.should_skip_schema(schema) && !self.config.should_skip_table(table)
        });
        info!("Processing {} pending data ...", pending_data.len());
        let pb = get_progress_bar(pending_data.len() as u64)?;
        let chunked_data = pending_data.into_iter().chunk_by(|data| data.xid);

        for (xid, group) in chunked_data.into_iter() {
            let mut tx = self.db.begin().await?;
            for data in group {
                match data.to_sql_inline() {
                    Ok(Some(query)) => {
                        sqlx::query(&query).execute(&mut *tx).await?;
                    }
                    Err(e) => {
                        error!("Failed to process pending data: {data:?}");
                        pb.finish_with_message("Failed");
                        return Err(e);
                    }
                    Ok(None) => {}
                }
                pb.inc(1);
            }
            pb.set_message(format!("Removing pending data for xid {}", xid));
            let tx = PendingData::remove_by_xid(tx, xid).await?;
            pb.set_message("Committing ...");
            tx.commit().await?;
            pb.finish_with_message("Done");
        }
        self.truncate_pending_data().await?;
        pb.finish_with_message("Replication completed");
        Ok(())
    }
}

fn extract_timestamp(mut entry: impl std::io::Read) -> MbLightResult<()> {
    let mut date_str = String::new();
    entry.read_to_string(&mut date_str)?;
    let date_str = date_str.trim();
    debug!("Raw timestamp: {:?}", date_str);

    // Append ":00" to make timezone compatible with %:z
    let date_str = if date_str.ends_with("+00") || date_str.ends_with("-00") {
        format!("{}:00", date_str)
    } else {
        date_str.to_string()
    };

    let date = DateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S%.f%:z")?.with_timezone(&Utc);
    let date = date.format("%Y-%m-%d %H:%M:%S");
    info!("Replication packet emitted at: {date}");
    Ok(())
}
