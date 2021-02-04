use std::cmp::min;
use std::convert::TryInto;
use std::fs;
use std::os::windows::fs::FileExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use dokan::{FindData, FindStreamData, OperationError, FileInfo};
use widestring::{U16CStr, U16CString};
use winapi::shared::ntstatus;
use winapi::shared::winerror;
use winapi::um::winnt;

use crate::bundles::database::{Database, DatabaseItem, ItemType, ChildIterator};
use crate::hashindex::HashedStr;
use super::{ReadOnlyFs,FsReadHandle};

pub struct BundleFs<'a> {
    database: Arc<Database<'a>>
}

impl<'a> BundleFs<'a> {
    pub fn new(database: Arc<Database<'a>>) -> BundleFs<'a> {
        BundleFs { database }
    }
}

impl<'ctx, 'fs: 'ctx> ReadOnlyFs for BundleFs<'fs> {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError> {
        let firstbs = path.find("\\");
        let deslashed_path = match firstbs {
            Some(0) => &path[1..],
            _ => path
        };
        let forwards_path = deslashed_path.replace('\\', "/");

        let (db_path, lang, extn) = split_path_to_key(&forwards_path);

        let item = self.database
            .get_by_str(db_path, lang, extn)
            .ok_or(OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND))?;
        
        match item.item_type() {
            ItemType::File => match stream {
                "" => return Ok(Arc::new(RawFileHandle::new(&item))),
                "raw" => return Ok(Arc::new(RawFileHandle::new(&item))),
                //"info" => return Ok(file_info_stream(item)),
                _ => Err(OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND))
            },
            ItemType::Folder => match stream {
                "" => return Ok(Arc::new(FolderHandle::new(&item))),
                //"info" => Ok(folder_info_stream(item)),
                _ => Err(OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND))
            }
        }
    }
}

fn split_last_dot(s: &str, limit: usize) -> (&str, &str) {
    match s.rfind('.') {
        None => (s, ""),
        Some(idx) => {
            if idx < limit {
                (s, "")
            }
            else {
                (&s[0..idx], &s[(idx+1)..])
            }
        }
    }
}

fn split_path_to_key(p: &str) -> (&str, &str, &str) {
    let last_slash = p.rfind('/').unwrap_or(0);
    let (remain, extn) = split_last_dot(p, last_slash);
    let (path, language) = split_last_dot(remain, last_slash);
    (path, language, extn)
}


fn key_to_name(key: &(HashedStr, HashedStr, HashedStr)) -> String {
    let path = format!("{}", key.0);
    let lang = format!("{}", key.1);
    let extn = format!("{}", key.2);

    let mut name = path.rsplit('/').next().unwrap().to_owned();
    let hasdot = name.contains(".");
    if lang.len() > 0 || hasdot {
        name += ".";
        name += &lang;
    }
    if extn.len() > 0 || hasdot {
        name += ".";
        name += &extn;
    }
    name
}

struct RawFileHandle {
    file_id: u64,
    storage_path: PathBuf,
    storage_offset: usize,
    length: usize,
    last_modified: SystemTime,
    backing_store: Mutex<Option<fs::File>>
}

impl RawFileHandle {
    fn new(item: &DatabaseItem) -> RawFileHandle {
        let back_deets = item.get_backing_details().unwrap();
        RawFileHandle {
            file_id: item.item_index() as u64,
            storage_path: back_deets.0.to_owned(),
            storage_offset: back_deets.1,
            length: back_deets.2,
            last_modified: item.last_modified(),
            backing_store: Mutex::new(None)
        }
    }
}

impl FsReadHandle for RawFileHandle {
    fn is_dir(&self) -> bool { false }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FindData>>, OperationError> {
        Err(OperationError::NtStatus(ntstatus::STATUS_NOT_A_DIRECTORY))
    }

    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        Ok(Box::new(vec![
            FindStreamData {
                name: widestring::U16CString::from_str(&"::$DATA").unwrap(),
                size: self.length.try_into().unwrap()
            },
            FindStreamData {
                name: widestring::U16CString::from_str(&":raw").unwrap(),
                size: self.length.try_into().unwrap()
            },
            FindStreamData {
                name: widestring::U16CString::from_str(&":info").unwrap(),
                size: 0
            }
        ].into_iter()))
    }

    fn get_file_info(&self) -> Result<FileInfo, OperationError> {
        Ok(FileInfo {
            attributes: winnt::FILE_ATTRIBUTE_READONLY,
            file_size: self.length as u64,
            file_index: self.file_id,
            creation_time: self.last_modified,
            last_write_time: self.last_modified,
            last_access_time: self.last_modified,
            number_of_links: 1
        })
    }

    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError> {
        let mut mg = self.backing_store.try_lock().unwrap();
        let backing = mg.get_or_insert_with(|| {
            let file_result = fs::File::open(&self.storage_path);
            match file_result {
                Ok(file) => file,
                // If opening fails, then the error is probably unrecoverable without
                // restarting anyway.
                Err(e) => panic!("Unable to read backing file {:?}: {}", self.storage_path, e)
            }
        });

        let read_from = self.storage_offset + (offset as usize);
        if read_from >= self.storage_offset + self.length {
            return Ok(0);
        }
        let amount_to_read = min(buf.len(), self.length - (offset as usize));
        if amount_to_read <= 0 { return Ok(0); }

        let capped_buf = &mut buf[0..(amount_to_read)];

        let res = backing.seek_read(capped_buf, read_from as u64);
        return res.or_else(|e| {
            let err = match e.raw_os_error(){
                Some(error) => error.try_into().unwrap(),
                None => winerror::ERROR_READ_FAULT
            };
            Err(OperationError::Win32(err))
        });
    }
}

struct FolderHandle {
    last_modified: SystemTime,
    items : Vec<FindData>
}
impl FolderHandle {
    fn new(item: &DatabaseItem) -> FolderHandle {
        let items : Vec<FindData> = item.children().map(|i| {
            let name = key_to_name(&i.key());
            let last_modified = i.last_modified();
            let attributes = match i.item_type() {
                ItemType::File => winnt::FILE_ATTRIBUTE_READONLY,
                ItemType::Folder => winnt::FILE_ATTRIBUTE_READONLY | winnt::FILE_ATTRIBUTE_DIRECTORY
            };

            FindData {
                file_name: U16CString::from_str(name).unwrap(),
                file_size: i.data_len() as u64,
                attributes,
                last_access_time: last_modified,
                last_write_time: last_modified,
                creation_time: last_modified
            }
        }).collect();
        FolderHandle {
            items,
            last_modified: item.last_modified()
        }
    }
}
impl FsReadHandle for FolderHandle {
    fn is_dir(&self) -> bool { true }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FindData>>, OperationError> {
        Ok(Box::new(self.items.clone().into_iter()))
    }
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> Result<usize, OperationError> { 
        Err(OperationError::NtStatus(ntstatus::STATUS_FILE_IS_A_DIRECTORY))
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        Ok(Box::new(vec![
            FindStreamData {
                name: widestring::U16CString::from_str(&":info").unwrap(),
                size: 0
            }
        ].into_iter()))
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