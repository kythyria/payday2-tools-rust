//! Implements the actual filesystem that the `mount` subcommand mounts and
//! the helpers to do the mounting.
//! 
//! The idea here is that we have a wrapper that turns a simple trait object
//! based interface into what dokan expects. That makes it easier for each part
//! of the FS to be a separate thing, at least to my C#-influenced brain.
//!
//! Note that to have a hope of file serial numbers being really unique, each
//! of the filesystems in here uses less than all 64 bits, so that the union
//! FS can use the top byte to indicate which layer it came from. Not really
//! all that important, but it's tidy.


use std::sync::Arc;
use std::time::SystemTime;

pub mod teststub;
pub mod raw_bundledb;
pub mod transcoder;

/// Trait of read-only filesystems
/// 
/// Deliberately minimal, much of the complexity in dokan only exists to
/// support writable filesystems.
pub trait ReadOnlyFs : Send + Sync {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, FsError>;
}

/// Trait of the handles from a read-only filesystem
/// 
/// This similarly returns trait objects for iterators to avoid the headache
/// that is passing a callback in, while also encapsulating what the iterator
/// really is.
pub trait FsReadHandle : Send + Sync {
    fn is_dir(&self) -> bool;
    fn len(&self) -> Option<usize>;
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, FsError>;
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError>;
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError>;
    fn get_file_info(&self) -> Result<FsFileInfo, FsError>;
}

#[derive(Clone)]
pub struct FsDirEntry {
    pub is_dir: bool,
    pub size: u64,
    pub modification_time: SystemTime,
    pub name: String
}

pub struct FsStreamEntry {
    pub size: i64,

    /// Name of stream, without the type or colons.
    pub name: String
}

pub struct FsFileInfo {
    /// The time when the file was created.
	pub creation_time: SystemTime,

	/// The time when the file was last accessed.
	pub last_access_time: SystemTime,

	/// The time when the file was last written to.
	pub last_write_time: SystemTime,

	/// Size of the file.
	pub file_size: u64,

	/// Number of hardlinks to the file.
	pub number_of_links: u32,

	/// The index that uniquely identifies the file in a volume.
	pub file_index: u64,

    pub is_dir: bool,

    pub read_only: bool,
}

#[derive(Debug)]
pub enum FsError {
    PastEnd,
    FileCorrupt,
    NotDirectory,
    IsDirectory,
    NotFound,
    ReadError,
    OsError(i32)
}
