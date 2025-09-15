use std::{
    fs::{self, File},
    io::{BufReader, Read},
};

use anyhow::Result;
use bytes::Bytes;
use bzip2::bufread::BzDecoder;
use futures_util::{SinkExt, StreamExt};
use musicbrainz_light_config::Settings;
use tar::Archive;
use tempfile::{NamedTempFile, TempDir};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_postgres::NoTls;

pub mod github_sync;
pub mod init;

const MB_DUMP: &str = "mbdump.tar.bz2";
const MB_DUMP_DERIVED: &str = "mbdump-derived.tar.bz2";
const COVER_ART_ARCHIVE: &str = "mbdump-cover-art-archive.tar.bz2";
const EVENT_ART_ARCHIVE: &str = "mbdump-even-art-archive.tar.bz2";
const MB_DUMP_STATS: &str = "mbdump-stats.tar.bz2";

const MUSICBRAINZ_FTP: &str = "http://ftp.musicbrainz.org/pub/musicbrainz/data/fullexport";

pub struct MusicBrainzLightDownloadClient {
    client: reqwest::Client,
    config: Settings,
    db: tokio_postgres::Client,
}

impl MusicBrainzLightDownloadClient {
    pub async fn new(config: Settings) -> Result<Self> {
        let conn_str = format!(
            "host={} port={} user={} password={} dbname={}",
            config.database.host,
            config.database.port,
            config.database.user,
            config.database.password,
            config.database.name
        );

        let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(Self {
            client: reqwest::Client::new(),
            config,
            db: client,
        })
    }

    pub async fn download_musicbrainz_data(&mut self) -> Result<()> {
        let mut filenames = vec![MB_DUMP, MB_DUMP_DERIVED, MB_DUMP_STATS];

        if !self
            .config
            .schema
            .ignore
            .contains(&"cover_art_archive".to_string())
        {
            filenames.push(COVER_ART_ARCHIVE);
        }
        if !self
            .config
            .schema
            .ignore
            .contains(&"event_art_archive".to_string())
        {
            filenames.push(EVENT_ART_ARCHIVE);
        }

        let latest = self.get_latest().await?;
        println!("Latest version: {}", latest);

        for filename in filenames {
            let url = format!("{}/{}/{}", MUSICBRAINZ_FTP, latest, filename);
            println!("Downloading {}", url);

            // Download bz2 into a temp file
            let response = self.client.get(&url).send().await?;
            let mut stream = response.bytes_stream();
            let tmpfile = NamedTempFile::new()?;
            let mut writer = tokio::fs::File::from_std(tmpfile.reopen()?);

            while let Some(chunk) = stream.next().await {
                let data = chunk?;
                writer.write_all(&data).await?;
            }
            writer.flush().await?;
            println!("Saved {} to {:?}", filename, tmpfile.path());

            // Use blocking tar+bz2
            let f = File::open(&tmpfile)?;
            let reader = BufReader::new(f);
            let decompressor = BzDecoder::new(reader);
            let mut archive = Archive::new(decompressor);

            // Iterate over archive entries
            for entry in archive.entries()? {
                let mut entry = entry?;
                let path = entry.path()?;
                let name = path.to_string_lossy();

                if !name.starts_with("mbdump/") {
                    continue;
                }

                let filename = name.strip_prefix("mbdump/").unwrap();
                let filename = filename.strip_suffix("_sanitised").unwrap_or(filename);

                let (schema, table) = filename
                    .split_once('.')
                    .unwrap_or(("musicbrainz", filename));

                if self.should_skip_table(schema, table).await? {
                    println!("Skipping {}", filename);
                    continue;
                }

                println!("Starting copy for table {}.{}", schema, table);
                let tx = self.db.transaction().await?;
                let sink = tx
                    .copy_in(&format!("COPY {}.{} FROM STDIN", schema, table))
                    .await?;
                tokio::pin!(sink);
                let mut sink = sink;

                // Stream data from tar entry into COPY
                let mut buffer = [0u8; 8192];
                loop {
                    let n = entry.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    sink.send(Bytes::copy_from_slice(&buffer[..n])).await?;
                }

                sink.finish().await?;
                tx.commit().await?;
            }
        }

        Ok(())
    }

    async fn should_skip_table(&self, schema: &str, table: &str) -> Result<bool> {
        if self.config.schema.ignore.contains(&schema.into()) {
            println!("Ignoring schema {}", schema);
            return Ok(true);
        }

        if self.config.tables.ignore.contains(&table.into()) {
            println!("Ignoring table {}.{}", schema, table);
            return Ok(true);
        }
        let fulltable = format!("{}.{}", schema, table);

        let table_exists: bool = self
            .db
            .query_one(
                "SELECT EXISTS (
                     SELECT FROM information_schema.tables
                     WHERE table_schema = $1 AND table_name = $2
                 )",
                &[&schema, &table],
            )
            .await?
            .get(0);

        if !table_exists {
            println!("Skipping {} (table {} does not exist)", table, fulltable);
            return Ok(true);
        }

        let has_data: bool = self
            .db
            .query_one(
                &format!("SELECT EXISTS (SELECT 1 FROM {} LIMIT 1)", fulltable),
                &[],
            )
            .await?
            .get(0);

        if has_data {
            println!(
                "Skipping {} (table {} already contains data)",
                table, fulltable
            );
            return Ok(true);
        }

        Ok(false)
    }

    async fn get_latest(&self) -> Result<String> {
        Ok(self
            .client
            .get(&format!("{}/LATEST", MUSICBRAINZ_FTP))
            .send()
            .await?
            .text()
            .await?
            .trim()
            .to_string())
    }
}
