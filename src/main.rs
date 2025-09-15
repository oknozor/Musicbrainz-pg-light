use std::path::PathBuf;

use crate::{pg::MusicBrainzLightDownloadClient, settings::Settings};
use anyhow::Result;

pub mod github_sync;
pub mod init;
pub mod pg;
pub mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::get()?;
    // let local_path = github_sync::download_musicbrainz_sql().await?;
    let local_path = PathBuf::from("/tmp/musicbrainz-sql");
    let mut client = MusicBrainzLightDownloadClient::new(settings).await?;
    client.create_schemas().await?;
    client.run_all_scripts(local_path).await?;
    client.download_musicbrainz_data().await?;
    Ok(())
}
