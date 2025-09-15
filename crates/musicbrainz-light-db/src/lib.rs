use musicbrainz_light_config::Settings;
use sqlx::PgPool;

pub async fn connect(config: &Settings) -> Result<sqlx::PgPool, sqlx::Error> {
    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.database.user,
        config.database.password,
        config.database.host,
        config.database.port,
        config.database.name
    );
    PgPool::connect(&url).await
}
