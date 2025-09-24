use std::{
    fs::File,
    io::{BufWriter, Write},
};

use crate::{
    MbLight,
    error::{MbLightError, MbLightResult},
    progress::get_progress_bar,
    settings::MbLightSettingsExt,
};
use futures_util::StreamExt;
use reqwest::StatusCode;

pub const MUSICBRAINZ_FTP: &str = "http://ftp.musicbrainz.org/pub/musicbrainz/data/fullexport";

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn get_latest(&self) -> MbLightResult<String> {
        Ok(self
            .http_client
            .get(format!("{}/LATEST", MUSICBRAINZ_FTP))
            .send()
            .await?
            .text()
            .await?
            .trim()
            .to_string())
    }

    pub async fn download_with_progress(
        &self,
        url: &str,
        tmpfile: &mut File,
    ) -> Result<(), MbLightError> {
        let response = self.http_client.get(url).send().await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(MbLightError::NotFound);
        }

        let total_size = response.content_length().unwrap_or(0);

        let pb = get_progress_bar(total_size)?;
        pb.set_message(format!("Downloading {}", url));

        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, tmpfile);
        let mut stream = response.bytes_stream();
        let mut buffered_progress: u64 = 0;
        let update_interval: u64 = 256 * 1024;

        while let Some(chunk) = stream.next().await {
            let data = chunk?;
            writer.write_all(&data)?;
            {
                buffered_progress += data.len() as u64;
                if buffered_progress >= update_interval {
                    pb.inc(buffered_progress);
                    buffered_progress = 0;
                }
            }
        }

        {
            if buffered_progress > 0 {
                pb.inc(buffered_progress);
            }

            pb.finish_with_message(format!("Downloaded {}", url));
        }
        Ok(())
    }
}
