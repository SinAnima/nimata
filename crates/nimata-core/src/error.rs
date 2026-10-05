use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(&'static str),

    #[error("{0}")]
    Invalid(String),

    #[error(
        "this database was created by a newer version of Nimata (schema {found}, supported {supported})"
    )]
    NewerSchema { found: i64, supported: i64 },

    #[error("the database is damaged: {0}")]
    Corrupt(String),

    #[error("{0} is not a Nimata database")]
    NotNimata(String),

    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
