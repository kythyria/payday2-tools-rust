use std::sync::Arc;
use std::convert::TryInto;
use std::io::Read;
use std::iter;
use std::time::SystemTime;

use super::{ReadOnlyFs, FsReadHandle, FsDirEntry, FsError, FsFileInfo, FsStreamEntry};

pub struct StaticFile<'a> {
    pub data: &'a [u8],
    pub timestamp: SystemTime,
    pub file_id: u64
}

impl<'a> FsReadHandle for StaticFile<'a> {
    fn is_dir(&self) -> bool { false }
    fn len(&self) -> Option<usize> { Some(self.data.len()) }
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, FsError> {
        let ofs: usize = offset.try_into().unwrap_or(usize::MAX);
        if ofs > self.data.len() {
            return Err(FsError::PastEnd);
        }
        let mut bs = &self.data[ofs..];
        bs.read(buf).or(Err(FsError::FileCorrupt))
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
        Err(FsError::NotDirectory)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        Ok(Box::new(vec![
            FsStreamEntry {
                name: String::from(""),
                size: self.data.len() as i64
            },
            FsStreamEntry {
                name: String::from("raw"),
                size: b"Hello World!".len() as i64
            }
        ].into_iter()))
    }
    fn get_file_info(&self) -> Result<FsFileInfo, FsError> {
        Ok(FsFileInfo {
            is_dir: false,
            read_only: true,
            file_size: self.data.len() as u64,
            file_index: self.file_id,
            creation_time: self.timestamp,
            last_write_time: self.timestamp,
            last_access_time: self.timestamp,
            number_of_links: 1
        })
    }
}

pub struct TestFs {

}

impl<'ctx, 'fs: 'ctx> ReadOnlyFs for TestFs {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, FsError> {
        if path == "\\test.txt" {
            Ok(Arc::new(StaticFile {
                data: match stream {
                    "" => b"Be alert, your country needs lerts.",
                    "raw" => b"Hello World!",
                    _ => return Err(FsError::NotFound)
                },
                timestamp: SystemTime::UNIX_EPOCH,
                file_id: 1
            }))
        }
        else if path == "\\" {
            Ok(Arc::new(TestDir { }))
        }
        else {
            Err(FsError::NotFound)
        }
    }
}

struct TestDir { }
impl super::FsReadHandle for TestDir {
    fn is_dir(&self) -> bool { true }
    fn len(&self) -> Option<usize> { None }
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> Result<usize, FsError> { 
        Err(FsError::IsDirectory)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        Err(FsError::IsDirectory)
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
        Ok(Box::new(iter::once(
            FsDirEntry {
                name: String::from("test.text"),
                is_dir: false,
                modification_time: SystemTime::UNIX_EPOCH,
                size: b"Be alert, your country needs lerts.".len() as u64
            }
        )))
    }
    fn get_file_info(&self) -> Result<FsFileInfo, FsError> {
        Ok(FsFileInfo {
            is_dir: true,
            read_only: true,
            file_size: 0,
            file_index: 0,
            creation_time: SystemTime::UNIX_EPOCH,
            last_write_time: SystemTime::UNIX_EPOCH,
            last_access_time: SystemTime::UNIX_EPOCH,
            number_of_links: 1
        })
    }
}