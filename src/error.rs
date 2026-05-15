use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShhError {
    #[error("invalid env name: {0}")]
    InvalidEnvName(String),
    #[error("invalid profile slug: {0}")]
    InvalidProfile(String),
    #[error("not found")]
    NotFound,
    #[error("keychain access denied or cancelled")]
    KeychainDenied,
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("dotenv parse error at line {line}: {message}")]
    DotenvParse { line: usize, message: String },
    #[error("cannot export {name} as dotenv: value contains characters this dotenv format cannot represent safely")]
    DotenvExport { name: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ShhError>;
