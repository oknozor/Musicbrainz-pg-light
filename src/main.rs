use std::path::PathBuf;

use anyhow::Result;
use musicbrainz_light_config::Settings;
use musicbrainz_light_download::{MusicBrainzLightDownloadClient, github_sync};

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
