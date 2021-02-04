use std::sync::Arc;
use std::convert::TryInto;
use std::io::Read;
use std::iter;
use std::time::SystemTime;

use dokan::*;
use widestring::{U16CStr, U16CString};
use winapi::shared::ntstatus;
use winapi::um::winnt;

use super::FsReadHandle;

pub struct StaticFile<'a> {
    pub data: &'a [u8],
    pub timestamp: SystemTime,
    pub file_id: u64
}

impl<'a> FsReadHandle for StaticFile<'a> {
    fn is_dir(&self) -> bool { false }
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError> {
        let ofs: usize = offset.try_into().unwrap_or(usize::MAX);
        if ofs > self.data.len() {
            return Err(OperationError::NtStatus(ntstatus::STATUS_BEYOND_VDL))
        }
        let mut bs = &self.data[ofs..];
        bs.read(buf).or(Err(OperationError::NtStatus(ntstatus::STATUS_FILE_CORRUPT_ERROR)))
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FindData>>, OperationError> {
        Err(OperationError::NtStatus(ntstatus::STATUS_NOT_A_DIRECTORY))
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        Ok(Box::new(vec![
            FindStreamData {
                name: widestring::U16CString::from_str(&"::$DATA").unwrap(),
                size: self.data.len() as i64
            },
            FindStreamData {
                name: widestring::U16CString::from_str(&":raw").unwrap(),
                size: b"Hello World!".len() as i64
            }
        ].into_iter()))
    }
    fn get_file_info(&self) -> Result<FileInfo, OperationError> {
        Ok(FileInfo {
            attributes: winnt::FILE_ATTRIBUTE_READONLY,
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

impl<'ctx, 'fs: 'ctx> super::ReadOnlyFs for TestFs {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError> {
        if path == "\\test.txt" {
            Ok(Arc::new(StaticFile {
                data: match stream {
                    "" => b"Be alert, your country needs lerts.",
                    "raw" => b"Hello World!",
                    _ => return Err(OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND))
                },
                timestamp: SystemTime::UNIX_EPOCH,
                file_id: 1
            }))
        }
        else if path == "\\" {
            Ok(Arc::new(TestDir { }))
        }
        else {
            Err(OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND))
        }
    }
}

struct TestDir { }
impl super::FsReadHandle for TestDir {
    fn is_dir(&self) -> bool { true }
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> Result<usize, OperationError> { 
        Err(OperationError::NtStatus(ntstatus::STATUS_FILE_IS_A_DIRECTORY))
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        Err(OperationError::NtStatus(ntstatus::STATUS_FILE_IS_A_DIRECTORY))
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FindData>>, OperationError> {
        Ok(Box::new(iter::once(
            FindData {
                file_name: U16CString::from_str(&"test.txt").unwrap(),
                attributes: winnt::FILE_ATTRIBUTE_READONLY,
                creation_time: SystemTime::UNIX_EPOCH,
                last_write_time: SystemTime::UNIX_EPOCH,
                last_access_time: SystemTime::UNIX_EPOCH,
                file_size: b"Be alert, your country needs lerts.".len() as u64
            }
        )))
    }
    fn get_file_info(&self) -> Result<FileInfo, OperationError> {
        Ok(FileInfo {
            attributes: winnt::FILE_ATTRIBUTE_READONLY | winnt::FILE_ATTRIBUTE_DIRECTORY,
            file_size: 0,
            file_index: 0,
            creation_time: SystemTime::UNIX_EPOCH,
            last_write_time: SystemTime::UNIX_EPOCH,
            last_access_time: SystemTime::UNIX_EPOCH,
            number_of_links: 1
        })
    }
}