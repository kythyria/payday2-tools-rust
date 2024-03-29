use std::cmp::min;
use std::convert::TryInto;
use std::fs;
use std::io::{prelude::*, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::bundles::database::{Database, DatabaseItem, HashStrKey, ItemType};
use crate::diesel_hash;
use super::{ReadOnlyFs, FsReadHandle, FsDirEntry, FsError, FsFileInfo, FsStreamEntry};

pub struct BundleFs{
    database: Arc<Database>
}

impl<'a> BundleFs {
    pub fn new(database: Arc<Database>) -> BundleFs {
        BundleFs { database }
    }
}

impl<'ctx, 'fs: 'ctx> ReadOnlyFs for BundleFs {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, FsError> {
        let firstbs = path.find("\\");
        let deslashed_path = match firstbs {
            Some(0) => &path[1..],
            _ => path
        };
        let forwards_path = deslashed_path.replace('\\', "/");

        let (db_path, lang, extn) = split_path_to_key(&forwards_path);

        let item = self.database
            .get_by_hashes(db_path, lang, extn)
            .ok_or(FsError::NotFound)?;
        
        match item.item_type() {
            ItemType::File => match stream {
                "" => return Ok(Arc::new(RawFileHandle::new(&item))),
                "raw" => return Ok(Arc::new(RawFileHandle::new(&item))),
                //"info" => return Ok(file_info_stream(item)),
                _ => Err(FsError::NotFound)
            },
            ItemType::Folder => match stream {
                "" => return Ok(Arc::new(FolderHandle::new(&item))),
                //"info" => Ok(folder_info_stream(item)),
                _ => Err(FsError::NotFound)
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

fn split_path_to_key(p: &str) -> (u64, u64, u64) {
    let last_slash = p.rfind('/').unwrap_or(0);
    let (remain, extn) = split_last_dot(p, last_slash);
    let (path, language) = split_last_dot(remain, last_slash);
    (diesel_hash::from_str(path), diesel_hash::from_str(language), diesel_hash::from_str(extn))
}


fn key_to_name(key: &HashStrKey) -> String {
    let path = format!("{}", key.path);
    let lang = format!("{}", key.language);
    let extn = format!("{}", key.extension);

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
    fn len(&self) -> Option<usize> { Some(self.length) }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        Ok(Box::new(std::iter::once(
            FsStreamEntry {
                name: "".into(),
                size: self.length.try_into().unwrap()
            }
        )))
    }

    fn get_file_info(&self) -> Result<FsFileInfo, FsError> {
        Ok(FsFileInfo {
            is_dir: false,
            read_only: true,
            file_size: self.length as u64,
            file_index: self.file_id,
            creation_time: self.last_modified,
            last_write_time: self.last_modified,
            last_access_time: self.last_modified,
            number_of_links: 1
        })
    }

    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, FsError> {
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

        let res = backing.seek(SeekFrom::Start(read_from as u64))
            .and_then(|_| backing.read(capped_buf));
        return res.map_err(|e| {
            match e.raw_os_error(){
                Some(error) => FsError::OsError(error),
                None => FsError::ReadError
            }
        });
    }
}

struct FolderHandle {
    last_modified: SystemTime,
    items : Vec<FsDirEntry>
}
impl FolderHandle {
    fn new(item: &DatabaseItem) -> FolderHandle {
        let items : Vec<FsDirEntry> = item.children().map(|i| {
            let name = key_to_name(&i.key());
            let modification_time = i.last_modified();

            FsDirEntry {
                name,
                modification_time,
                is_dir: match i.item_type() { ItemType::File => false, ItemType::Folder => true },
                size: i.data_len() as u64
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
    fn len(&self) -> Option<usize> { None }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
        Ok(Box::new(self.items.clone().into_iter()))
    }
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> Result<usize, FsError> {
        Err(FsError::IsDirectory)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        Ok(Box::new(std::iter::once(
            FsStreamEntry {
                name: String::from("info"),
                size: 0
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