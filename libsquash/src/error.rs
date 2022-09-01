use std::io;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Invalid value: {0}")]
    Format(String),
    #[error("Value out of bounds: {0}")]
    Bounds(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
