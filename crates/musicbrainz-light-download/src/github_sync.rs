use anyhow::Result;
use futures_util::future::join_all;
use octocrab::Octocrab;
use std::fs;
use tempfile::env::temp_dir;

pub async fn download_musicbrainz_sql() -> Result<PathBuf> {
    let octocrab = Octocrab::builder().build()?;
    let client = reqwest::Client::new();
    let owner = "metabrainz";
    let repo = "musicbrainz-server";
    let path = "admin/sql";
    let local_dir = temp_dir();
    let local_dir = local_dir.join("musicbrainz-sql");
    download_dir(
        client,
        octocrab,
        owner.into(),
        repo.into(),
        path.into(),
        local_dir.clone(),
    )
    .await?;

    Ok(local_dir)
}

use std::path::PathBuf;

async fn download_dir(
    client: reqwest::Client,
    octocrab: Octocrab,
    owner: String,
    repo: String,
    path: String,
    local_path: PathBuf,
) -> Result<()> {
    fs::create_dir_all(&local_path)?;

    let contents = octocrab
        .repos(&owner, &repo)
        .get_content()
        .path(path)
        .send()
        .await?
        .items;

    let mut files = vec![];
    for item in contents {
        let item_path = local_path.join(&item.name);
        let client = client.clone();

        match item.r#type.as_str() {
            "dir" => {
                let octocrab = octocrab.clone();
                let owner = owner.clone();
                let repo = repo.clone();
                let path = item.path.clone();
                let local_path = item_path.clone();

                Box::pin(download_dir(
                    client, octocrab, owner, repo, path, local_path,
                ))
                .await?;
            }
            "file" => {
                if let Some(download_url) = item.download_url {
                    let fut = async move {
                        let file_path = item_path.clone();
                        let bytes = client.get(&download_url).send().await?.bytes().await?;
                        tokio::fs::write(&file_path, &bytes).await?;
                        println!("Downloaded {}", file_path.display());
                        anyhow::Ok(())
                    };

                    files.push(fut);
                }
            }
            _ => {}
        }
    }

    let results = join_all(files).await;
    for r in results {
        r?;
    }

    Ok(())
}
