pub mod bundledb_reader;
pub mod packageheader_reader;
pub mod database;
pub mod loader;

#[derive(Debug)]
pub enum ReadError {
    UnknownFormatOrMalformed,
    IoError(std::io::Error),
    ParseFailed(String),
    BadMultiBundleHeader
}
variant_from!(ReadError::IoError, std::io::Error);