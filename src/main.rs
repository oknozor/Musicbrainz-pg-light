use std::time::{Duration, Instant};

use crate::{restore::MbLight, settings::Settings};
use anyhow::Result;

pub mod github_sync;
pub mod init;
pub mod restore;
pub mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    let start = Instant::now();
    let settings = Settings::get()?;
    let local_path = github_sync::download_musicbrainz_sql().await?;
    let mut client = MbLight::new(settings).await?;
    client.create_schemas().await?;
    client.create_tables(&local_path).await?;
    client.ingest_musicbrainz_data().await?;
    client.run_all_scripts(local_path).await?;
    let duration = start.elapsed();
    println!(
        "Job finished, took time: {:?}",
        format_minutes_seconds(duration)
    );
    Ok(())
}

fn format_minutes_seconds(elapsed: Duration) -> String {
    let total_seconds = elapsed.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
