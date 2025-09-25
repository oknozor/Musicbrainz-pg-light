use std::{sync::Arc, time::Duration};

use crate::musicbrainz_db::replication::replication_control::ReplicationControl;
use crate::{error::MbLightResult, settings::MbLightSettingsExt};
use octocrab::Octocrab;
use sqlx::PgPool;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

mod error;
mod tar_helper;

pub(crate) mod download;
pub(crate) mod musicbrainz_db;
pub(crate) mod progress;

pub mod settings;

pub use error::MbLightError;

pub struct MbLight<S: MbLightSettingsExt> {
    pub http_client: reqwest::Client,
    pub github_client: Octocrab,
    pub config: Arc<S>,
    pub db: PgPool,
    pub reindex_sender: Option<Sender<()>>,
}

impl<S: MbLightSettingsExt> MbLight<S> {
    pub fn try_new(config: S, db: PgPool) -> Result<Self, MbLightError> {
        Ok(Self {
            http_client: reqwest::Client::new(),
            config: Arc::new(config),
            db,
            github_client: Octocrab::builder().build()?,
            reindex_sender: None,
        })
    }

    pub fn with_sender(mut self, sender: Sender<()>) -> Self {
        self.reindex_sender = Some(sender);
        self
    }

    /// Initialize the database by downloading and processing MusicBrainz SQL dump.
    pub async fn init(&mut self) -> MbLightResult<()> {
        let local_path = self.download_musicbrainz_sql().await?;
        self.create_schemas().await?;
        self.create_tables(&local_path).await?;
        self.ingest_dump().await?;
        self.run_all_scripts(local_path).await?;
        Ok(())
    }

    pub async fn sync(&self, infinite: bool) -> Result<(), MbLightError> {
        self.drop_tablecheck().await?;
        loop {
            match self.apply_pending_replication().await {
                Ok(_) => {}
                Err(MbLightError::NotFound) => {
                    if let Some(sender) = &self.reindex_sender {
                        info!("Reached last replication packet, sending reindex signal");
                        sender.send(()).await?;
                    }
                    if infinite {
                        info!("Waiting for 15 minutes for a fresh replication packet");
                        tokio::time::sleep(Duration::from_secs(60 * 15)).await;
                    } else {
                        let control = ReplicationControl::get(&self.db).await?;
                        info!(
                            "Reached last replication packet, schema_sequence = {}, replication_sequence = {}, terminating",
                            control.current_schema_sequence.expect("schema sequence"),
                            control
                                .current_replication_sequence
                                .expect("replication sequence")
                        );
                        return Ok(());
                    }
                }
                Err(err) => {
                    error!("Fatal error applying pending replication: {}", err);
                    return Err(err);
                }
            }
        }
    }

    pub async fn has_data(&self, schema: &str, table: &str) -> MbLightResult<bool> {
        let fulltable = format!("{}.{}", schema, table);

        let has_data: bool = sqlx::query_scalar(&format!(
            "SELECT EXISTS (SELECT 1 FROM {} LIMIT 1)",
            fulltable
        ))
        .fetch_one(&self.db)
        .await
        .map(Option::unwrap_or_default)?;

        Ok(has_data)
    }
}
