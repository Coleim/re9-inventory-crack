use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not a DSSS save: {0}")]
    Format(String),

    #[error("decrypt failed: {0}")]
    Decrypt(String),

    #[error("could not recover a SteamID by brute force in the account-id space")]
    CrackFailed,

    #[error("offset {off:#x} out of bounds (length {len})")]
    OutOfBounds { off: usize, len: usize },

    #[error("{0}")]
    Edit(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Decrypt(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Decrypt(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
