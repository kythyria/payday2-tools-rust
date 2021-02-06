use std::sync::Arc;

use dokan::*;

use super::{ReadOnlyFs, FsReadHandle, FsDirEntry};

pub(super) struct TranscoderFs<'a> {
    backing: Arc<dyn ReadOnlyFs + 'a>
}

impl<'a> TranscoderFs<'a> {
    pub(super) fn new(backing: Arc<dyn ReadOnlyFs + 'a>) -> TranscoderFs<'a> {
        TranscoderFs {
            backing
        }
    }
}

impl ReadOnlyFs for TranscoderFs<'_> {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError> {
        let mut real_path = path.to_owned();
        if real_path.ends_with(".dds") {
            real_path.truncate(real_path.len() - ".dds".len());
            real_path.push_str(".texture");
        }
        else if real_path.ends_with(".bik") {
            real_path.truncate(real_path.len() - ".bik".len());
            real_path.push_str(".movie");
        }

        let backing_handle = self.backing.open_readable(&real_path, stream)?;
        if backing_handle.is_dir() {
            Ok(Arc::new(FolderHandle { backing: backing_handle }))
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
            if newname.ends_with(".texture") {
                newname.truncate(newname.len() - ".texture".len());
                newname.push_str(".dds");
            }
            else if newname.ends_with(".movie") {
                newname.truncate(newname.len() - ".movie".len());
                newname.push_str(".bik");
            };

            FsDirEntry {
                name: newname,
                ..fd
            }
        })))
    }
}