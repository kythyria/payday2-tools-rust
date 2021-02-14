use std::convert::TryInto;
use std::io::Read;
use std::sync::Arc;
use std::time::SystemTime;

use dokan::*;
use winapi::shared::ntstatus;
use winapi::um::winnt;

use crate::hashindex::HashIndex;
use super::{ReadOnlyFs, FsReadHandle, FsDirEntry};

pub(super) struct TranscoderFs<'a> {
    hashlist: Arc<HashIndex>,
    backing: Arc<dyn ReadOnlyFs + 'a>
}

impl<'a> TranscoderFs<'a> {
    pub(super) fn new(hashlist: Arc<HashIndex>, backing: Arc<dyn ReadOnlyFs + 'a>) -> TranscoderFs<'a> {
        TranscoderFs {
            hashlist,
            backing
        }
    }
}

impl ReadOnlyFs for TranscoderFs<'_> {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError> {
        let mut real_path = path.to_owned();
        let maybe_rule = TRANSCODE_RULES.iter().find(|i| real_path.ends_with(i.displayed_extension));
        match maybe_rule {
            None => (),
            Some(rule) => {
                real_path.truncate(real_path.len() - rule.displayed_extension.len());
                real_path.push_str(rule.backing_extension);
            }
        }

        let backing_handle = self.backing.open_readable(&real_path, if stream == "raw" { "" } else { stream })?;
        if backing_handle.is_dir() {
            Ok(Arc::new(FolderHandle { backing: backing_handle }))
        }
        else if stream == "" {
            if let Some(converter) = maybe_rule.map(|r| r.transformer).flatten() {
                let info = backing_handle.get_file_info().unwrap();
                let mut back_buf = Vec::<u8>::new();
                back_buf.resize(info.file_size as usize, 0);
                backing_handle.read_at(&mut back_buf, 0)?;
                let front_buf = converter(&self.hashlist, &back_buf);

                let front_handle = VecFileHandle {
                    data: front_buf,
                    timestamp: info.creation_time,
                    file_id: info.file_index
                };

                Ok(Arc::new(front_handle))
            }
            else {
                Ok(backing_handle)
            }
        }
        else {
            Ok(backing_handle)
        }
    }
}

struct FolderHandle {
    backing: Arc<dyn FsReadHandle>
}

impl FsReadHandle for FolderHandle {
    fn is_dir(&self) -> bool { true }
    fn len(&self) -> Option<usize> { self.backing.len() }
    fn get_file_info(&self) -> Result<FileInfo, OperationError> {
        self.backing.get_file_info()
    }
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError> {
        self.backing.read_at(buf, offset)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        self.backing.list_streams()
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, OperationError> {
        let backing_iter = self.backing.find_files()?;
        Ok(Box::new(backing_iter.map(|fd| {
            let mut newname = String::from(fd.name);
            for rule in TRANSCODE_RULES.iter() {
                if  newname.ends_with(rule.backing_extension) {
                    newname.truncate(newname.len() - rule.backing_extension.len());
                    newname.push_str(rule.displayed_extension);
                    break;
                }
            }

            FsDirEntry {
                name: newname,
                ..fd
            }
        })))
    }
}

struct VecFileHandle {
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
    pub file_id: u64
}

impl FsReadHandle for VecFileHandle {
    fn is_dir(&self) -> bool { false }
    fn len(&self) -> Option<usize> { Some(self.data.len()) }
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError> {
        let ofs: usize = offset.try_into().unwrap_or(usize::MAX);
        if ofs > self.data.len() {
            return Err(OperationError::NtStatus(ntstatus::STATUS_BEYOND_VDL))
        }
        let mut bs = &self.data[ofs..];
        bs.read(buf).or(Err(OperationError::NtStatus(ntstatus::STATUS_FILE_CORRUPT_ERROR)))
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, OperationError> {
        Err(OperationError::NtStatus(ntstatus::STATUS_NOT_A_DIRECTORY))
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError> {
        Ok(Box::new(vec![
            FindStreamData {
                name: widestring::U16CString::from_str(&"::$DATA").unwrap(),
                size: self.data.len() as i64
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

macro_rules! struct_from_tuple_table {
    (@make_row $sn:ident {$($sin:ident : $sit:ty),*} ($($ri:expr),*) ) => {
        $sn { $($sin: $ri,)* }
    };
    (@make_table $sn:ident $sb:tt [ $($row:tt),* ]) => {
        [ $(struct_from_tuple_table!(@make_row $sn $sb $row),)* ]
    };
    ($struct_name:ident $struct_body:tt $($cname:ident = $cbody:tt)* ) => {
        struct $struct_name $struct_body
        $(
            const $cname : &[$struct_name] = & struct_from_tuple_table!(@make_table $struct_name $struct_body $cbody);
        )*
    }
}

struct_from_tuple_table! {
    TranscodeRule {
        backing_extension: &'static str,
        displayed_extension: &'static str,
        hide_original: bool,
        transformer: Option<fn(&HashIndex, &[u8]) -> Vec<u8>>
    }

    TRANSCODE_RULES = [
        (".movie",   ".bik", false, None),
        (".texture", ".dds", false, None),
        (".strings", ".strings.json", false, Some(transcode_strings))
    ]
}

fn transcode_strings(hi: &HashIndex, input: &[u8]) -> Vec<u8> {
    let mut buf = Vec::<u8>::with_capacity(input.len());
    crate::formats::string_table::bytes_to_json(hi, input, &mut buf).unwrap();
    buf
}