use clap::Parser;
use color_eyre::{Result, config::HookBuilder};
use musicbrainz_light::{MbLight, settings::Settings};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
pub enum Cli {
    /// Initialize the database
    Init,
    /// Sync the database with the latest MusicBrainz data
    Sync {
        /// Wait for the next replication packet infinitely
        #[arg(long, short)]
        r#loop: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    HookBuilder::default()
        .panic_section(
            "If you believe this is a bug, please file an issue at: https://github.com/oknozor/mbpg-light/issues\n\
             Include a minimal reproduction and the output below."
        )
        .install()?;
    let indicatif_layer = IndicatifLayer::new();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "mbpg_light=info,musicbrainz_light=info".into()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .with_writer(indicatif_layer.get_stderr_writer()),
        )
        .with(indicatif_layer)
        .init();

    let cli = Cli::parse();
    let config = Settings::get()?;
    let db_url = config.db_url();

    let mut mblight = MbLight::try_new(config, db_url).await?;

    match cli {
        Cli::Init => mblight.init().await?,
        Cli::Sync { r#loop } => mblight.sync(r#loop).await?,
    }

    Ok(())
}
