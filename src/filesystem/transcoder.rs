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
        for i_t in TRANSCODE_RULES.iter() {
            let rule = TranscodeRule2::from(i_t);
            if real_path.ends_with(rule.displayed_extension) {
                real_path.truncate(real_path.len() - rule.displayed_extension.len());
                real_path.push_str(rule.backing_extension);
                break;
            }
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
            for i_t in TRANSCODE_RULES.iter() {
                let rule = TranscodeRule2::from(i_t);
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

macro_rules! struct_from_tuple_table {
    (@make_tuple { $( $idx:tt : $n:ident : $t:ty),* }) => { ( $($t,)* ) };
    (@make_struct $name:ident { $( $idx:tt : $n:ident : $t:ty),* }) => {
        struct $name {
            $( $n : $t , )*
        }
        impl std::convert::From< &( $($t,)* ) > for $name {
            fn from(t: &( $($t,)* ) ) -> $name {
                $name {
                    $( $n: t.$idx , )*
                }
            }
        }
    };
    ($name:ident $body:tt $($cname:ident = $cbody:tt)* ) => {
        struct_from_tuple_table!(@make_struct $name $body);
        $(
            const $cname : &[struct_from_tuple_table!(@make_tuple $body)] = & $cbody ;
        )*
    };
}

struct_from_tuple_table! {
    TranscodeRule2 {
        0: backing_extension: &'static str,
        1: displayed_extension: &'static str,
        2: hide_original: bool,
        3: transformer: Option<fn(&[u8]) -> Vec<u8>>
    }

    TRANSCODE_RULES = [
        (".movie",   ".bik", false, None),
        (".texture", ".dds", false, None),
    ]
}