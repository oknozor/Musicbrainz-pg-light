use crate::error::MbLightResult;
use crate::progress::get_progress_bar;
use crate::settings::MbLightSettingsExt;
use crate::{MbLight, MbLightError};
use futures_util::future::join_all;
use indicatif::MultiProgress;
use std::fs;
use std::path::PathBuf;
use tempfile::env::temp_dir;
use tracing::error;

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn download_musicbrainz_sql(&self) -> MbLightResult<PathBuf> {
        let owner = "metabrainz";
        let repo = "musicbrainz-server";
        let path = "admin/sql";
        let local_dir = temp_dir();
        let local_dir = local_dir.join("musicbrainz-sql");

        let mp = MultiProgress::new();
        self.download_dir(
            owner.into(),
            repo.into(),
            path.into(),
            local_dir.clone(),
            mp.clone(),
        )
        .await?;

        mp.clear()?;
        Ok(local_dir)
    }

    pub async fn download_schema_update(&self, target_sequence: i32) -> MbLightResult<PathBuf> {
        let owner = "metabrainz";
        let repo = "musicbrainz-server";
        let path = format!("admin/sql/update/schema-change/{}.all.sql", target_sequence);
        let local_dir = temp_dir();
        let local_dir = local_dir.join("musicbrainz-sql");

        let mp = MultiProgress::new();
        let path_clone = PathBuf::from(&path);
        self.download_dir(
            owner.into(),
            repo.into(),
            path,
            local_dir.clone(),
            mp.clone(),
        )
        .await?;

        mp.clear()?;
        Ok(path_clone)
    }

    async fn download_dir(
        &self,
        owner: String,
        repo: String,
        path: String,
        local_path: PathBuf,
        mp: MultiProgress,
    ) -> MbLightResult<()> {
        fs::create_dir_all(&local_path)?;

        let contents = self
            .github_client
            .repos(&owner, &repo)
            .get_content()
            .path(path)
            .send()
            .await?
            .items;

        let pb = mp.add(get_progress_bar(contents.len() as u64)?);
        pb.set_message(format!(
            "Dir {}",
            local_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        let mut files = vec![];
        for item in contents {
            let item_path = local_path.join(&item.name);
            let mp = mp.clone();
            let pb = pb.clone();

            match item.r#type.as_str() {
                "dir" => {
                    let owner = owner.clone();
                    let repo = repo.clone();
                    let path = item.path.clone();
                    let local_path = item_path.clone();

                    Box::pin(self.download_dir(owner, repo, path, local_path, mp)).await?;
                    pb.inc(1);
                }
                "file" => {
                    if let Some(download_url) = item.download_url {
                        let fut = async move {
                            let file_path = item_path.clone();
                            let bytes = self
                                .http_client
                                .get(&download_url)
                                .send()
                                .await?
                                .bytes()
                                .await?;
                            tokio::fs::write(&file_path, &bytes).await?;

                            pb.inc(1);
                            Ok::<(), MbLightError>(())
                        };

                        files.push(fut);
                    } else {
                        pb.inc(1);
                    }
                }
                _ => {}
            }
        }

        let results = join_all(files).await;
        for r in results {
            if let Err(e) = r {
                error!("Error: {}", e);
            }
        }
        pb.finish_with_message("Download complete");

        Ok(())
    }
}
