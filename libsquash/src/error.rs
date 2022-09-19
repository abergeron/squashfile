use std::io;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Crypto error")]
    Crypto(&'static str),
    #[error("Decompression error")]
    Compression(&'static str),
    #[error("Invalid value: {0}")]
    Format(&'static str),
    #[error("Value out of bounds: {0}")]
    Bounds(&'static str),
    #[error("Invalid operation: {0}")]
    InvalidOperation(&'static str),
}
