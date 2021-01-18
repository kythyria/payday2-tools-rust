pub mod bundledb_reader;
pub mod packageheader_reader;
pub mod database;
pub mod loader;

use std::io::Error as IoError;

#[derive(Debug)]
pub enum ReadError {
    UnknownFormatOrMalformed,
    IoError(std::io::Error),
    ParseFailed(String),
    BadMultiBundleHeader
}

impl std::convert::From<IoError> for ReadError {
    fn from(e: IoError) -> ReadError { ReadError::IoError(e) }
}