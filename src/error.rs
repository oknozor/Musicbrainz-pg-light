use indicatif::style::TemplateError;

pub type MbLightResult<T> = Result<T, MbLightError>;

#[derive(Debug, thiserror::Error)]
pub enum MbLightError {
    #[error("Next replication packet not found")]
    NotFound,
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Database error: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("Send error: {0}")]
    Send(#[from] tokio::sync::mpsc::error::SendError<()>),
    #[error("Replication sequence missmatch, expected {expected} but got {got}")]
    SequenceMissmatch { expected: i32, got: i32 },
    #[error("Replication schema missmatch, expected {expected} but got {got}")]
    SchemaMissmatch { expected: i32, got: i32 },
    #[error("Progress bar template error: {0}")]
    ProgressBarTemplate(#[from] TemplateError),
    #[error("Github client error: {0}")]
    GithubClient(#[from] octocrab::Error),
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("Date parse error: {0}")]
    DateParseError(#[from] chrono::ParseError),
    #[error("Missing pending data {0}")]
    MissingPendingData(&'static str),
    #[error("Malformed pending data {0}")]
    MalformedPendingData(&'static str),
    #[error("No replication sequence in 'replication_control' table")]
    MissingRepplicationSequence,
}
