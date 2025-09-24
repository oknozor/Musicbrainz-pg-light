use std::{sync::Arc, time::Duration};

use octocrab::Octocrab;
use sqlx::PgPool;
#[cfg(feature = "notify")]
use tokio::sync::mpsc::Sender;
use tokio::time;
use tracing::{error, info};

use crate::{error::MbLightResult, settings::MbLightSettingsExt};

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
    #[cfg(feature = "notify")]
    pub reindex_sender: Sender<()>,
}

impl<S: MbLightSettingsExt> MbLight<S> {
    pub fn try_new(
        config: S,
        db: PgPool,
        #[cfg(feature = "notify")] reindex_sender: Sender<()>,
    ) -> Result<Self, MbLightError> {
        Ok(Self {
            http_client: reqwest::Client::new(),
            config: Arc::new(config),
            db,
            #[cfg(feature = "notify")]
            reindex_sender,
            github_client: Octocrab::builder().build()?,
        })
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

    pub async fn sync(&self) -> Result<(), MbLightError> {
        self.drop_tablecheck().await?;
        loop {
            match self.apply_pending_replication().await {
                Ok(_) => {}
                Err(MbLightError::NotFound) => {
                    #[cfg(feature = "notify")]
                    {
                        info!("Reached last replication packet, sending reindex signal");
                        self.reindex_sender.send(()).await?;
                    }
                    info!("Waiting for 15 minutes for a fresh replication packet");
                    time::sleep(Duration::from_secs(60 * 15)).await;
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
