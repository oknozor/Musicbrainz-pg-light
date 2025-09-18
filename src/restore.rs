use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    str::FromStr,
};

use crate::settings::Settings;
use anyhow::{Context, Result};
use bb8_postgres::{
    PostgresConnectionManager,
    bb8::{Pool, PooledConnection},
};
use bytes::Bytes;
use bzip2::bufread::BzDecoder;
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use tar::{Archive, Entry};
use tempfile::NamedTempFile;
use tokio_postgres::{CopyInSink, NoTls};

const MB_DUMP: &str = "mbdump.tar.bz2";
const MB_DUMP_DERIVED: &str = "mbdump-derived.tar.bz2";
const COVER_ART_ARCHIVE: &str = "mbdump-cover-art-archive.tar.bz2";
const EVENT_ART_ARCHIVE: &str = "mbdump-even-art-archive.tar.bz2";
const MB_DUMP_STATS: &str = "mbdump-stats.tar.bz2";

const MUSICBRAINZ_FTP: &str = "http://ftp.musicbrainz.org/pub/musicbrainz/data/fullexport";

pub struct MbLight {
    pub client: reqwest::Client,
    pub config: Settings,
    pub pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl MbLight {
    pub async fn new(config: Settings) -> Result<Self> {
        let conn_str = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.database.user,
            config.database.password,
            config.database.host,
            config.database.port,
            config.database.name
        );

        let pg_config = tokio_postgres::config::Config::from_str(&conn_str)?;
        let manager = PostgresConnectionManager::new(pg_config, tokio_postgres::NoTls);
        let pool = Pool::builder().build(manager).await?;

        Ok(Self {
            client: reqwest::Client::new(),
            config,
            pool,
        })
    }

    pub async fn ingest_musicbrainz_data(&mut self) -> Result<()> {
        let mut filenames = vec![MB_DUMP, MB_DUMP_DERIVED];

        if !self.config.schema.should_skip("statistics") {
            filenames.push(MB_DUMP_STATS);
        }
        if !self.config.schema.should_skip("cover_art_archive") {
            filenames.push(COVER_ART_ARCHIVE);
        }
        if !self.config.schema.should_skip("event_art_archive") {
            filenames.push(EVENT_ART_ARCHIVE);
        }

        let latest = self.get_latest().await?;
        println!("Latest version: {}", latest);

        for filename in filenames {
            let url = format!("{}/{}/{}", MUSICBRAINZ_FTP, latest, filename);
            let tempfile = NamedTempFile::new()?;
            let mut writer = tempfile.reopen()?;
            self.download_with_progress(&url, &mut writer).await?;
            let mut archive = get_archive(tempfile.path())?;
            let pool = self.pool.clone();
            let config = self.config.clone();

            println!("Starting pg_copy for {filename}");

            let mut db = pool.get().await?;
            for entry in archive.entries()? {
                match entry {
                    Ok(entry) => {
                        let path = entry.path()?;
                        let entry_size = entry.header().entry_size()?;
                        let name = path.to_string_lossy().into_owned();

                        if !name.starts_with("mbdump/") {
                            continue;
                        }

                        let filename = name.strip_prefix("mbdump/").unwrap();
                        let filename = filename.strip_suffix("_sanitised").unwrap_or(filename);

                        let (schema, table) = filename
                            .split_once('.')
                            .unwrap_or(("musicbrainz", filename));

                        if should_skip_table(&config, &db, schema, table).await? {
                            continue;
                        }

                        let pb = ProgressBar::new(entry_size);
                        pb.set_style(
                                                        ProgressStyle::default_bar()
                                                            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) - {msg}")
                                                            .unwrap()
                                                            .progress_chars("#>-"),
                                                    );
                        pb.set_message(table.to_string());
                        pg_copy(&mut db, entry, schema, table, pb)
                            .await
                            .context(format!("in {schema}.{table}"))?;
                    }
                    Err(err) => {
                        eprintln!("{err}");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_latest(&self) -> Result<String> {
        Ok(self
            .client
            .get(format!("{}/LATEST", MUSICBRAINZ_FTP))
            .send()
            .await?
            .text()
            .await?
            .trim()
            .to_string())
    }

    async fn download_with_progress(&self, url: &str, tmpfile: &mut File) -> anyhow::Result<()> {
        let response = self.client.get(url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n - [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(format!("Downloading {}", url));
        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, tmpfile);
        let mut stream = response.bytes_stream();
        let mut buffered_progress: u64 = 0;
        let update_interval: u64 = 256 * 1024;
        while let Some(chunk) = stream.next().await {
            let data = chunk?;
            writer.write_all(&data)?;
            buffered_progress += data.len() as u64;
            if buffered_progress >= update_interval {
                pb.inc(buffered_progress);
                buffered_progress = 0;
            }
        }

        if buffered_progress > 0 {
            pb.inc(buffered_progress);
        }

        pb.finish_with_message(format!("Downloaded {}", url));
        Ok(())
    }
}

pub async fn pg_copy(
    db: &mut PooledConnection<'_, PostgresConnectionManager<NoTls>>,
    mut entry: Entry<'_, impl Read>,
    schema: &str,
    table: &str,
    pb: ProgressBar,
) -> Result<(), anyhow::Error> {
    let mut debug_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/musicbrainz_tag_debug.tbl")?;
    db.execute(
        &format!("ALTER TABLE {}.{} SET UNLOGGED", schema, table),
        &[],
    )
    .await
    .context("Failed to set table to UNLOGGED")?;

    let tx = db
        .transaction()
        .await
        .context("Failed to start transaction")?;

    let sink: CopyInSink<Bytes> = tx
        .copy_in(&format!("COPY {}.{} FROM STDIN", schema, table))
        .await
        .context("Failed to start COPY")?;
    tokio::pin!(sink);

    let mut buffer = vec![0u8; 8 * 1024 * 1024];

    loop {
        let n = entry
            .read(&mut buffer)
            .context("Failed to read from archive entry")?;
        if n == 0 {
            break;
        }

        let chunk = Bytes::copy_from_slice(&buffer[..n]);
        debug_file.write_all(&chunk)?;
        sink.send(chunk)
            .await
            .context("Failed to send data chunk to Postgres")?;

        pb.inc(n as u64);
    }

    sink.finish().await.context("Failed to close sink")?;

    pb.set_message(format!("Committing on {schema}.{table}"));
    tx.commit().await.context("Failed to commit transaction")?;
    db.execute(&format!("ALTER TABLE {}.{} SET LOGGED", schema, table), &[])
        .await
        .context("Failed to restore LOGGED on table")?;

    pb.finish_with_message(format!("{schema}.{table} COPY done!"));
    Ok(())
}

fn get_archive(tmpfile: &Path) -> Result<Archive<impl Read>> {
    let f = File::open(tmpfile)?;
    let reader = BufReader::new(f);
    let decompressor = BzDecoder::new(reader);
    let archive = Archive::new(decompressor);
    Ok(archive)
}

async fn should_skip_table(
    config: &Settings,
    db: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<bool> {
    if config.schema.should_skip(schema) {
        return Ok(true);
    }

    if config.tables.should_skip(table) {
        return Ok(true);
    }
    let fulltable = format!("{}.{}", schema, table);

    let table_exists: bool = db
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

    let has_data: bool = db
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
