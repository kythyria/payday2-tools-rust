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

use std::convert::TryInto;
use std::sync::Arc;

use dokan::*;
use widestring::{U16CString, U16CStr};
use winapi::um::winnt;
use winapi::shared::ntstatus;

mod teststub;
mod raw_bundledb;
mod router;

/// Trait of read-only filesystems
/// 
/// Deliberately minimal, much of the complexity in dokan only exists to
/// support writable filesystems.
trait ReadOnlyFs : Send + Sync {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError>;
}

/// Trait of the handles from a read-only filesystem
/// 
/// This similarly returns trait objects for iterators to avoid the headache
/// that is passing a callback in, while also encapsulating what the iterator
/// really is.
trait FsReadHandle : Send + Sync {
    fn is_dir(&self) -> bool;
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError>;
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FindData>>, OperationError>;
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FindStreamData>>, OperationError>;
    fn get_file_info(&self) -> Result<FileInfo, OperationError>;
}

struct DokanAdapter {
    fs: Arc<dyn ReadOnlyFs>,
    serial: u32,
    name: U16CString
}

pub struct AdapterContext {
    handle: Arc<dyn FsReadHandle>
}

impl<'c, 's: 'c> FileSystemHandler<'c, 's> for DokanAdapter {
    type Context = AdapterContext;

    fn get_volume_information(&self, _info: &OperationInfo<Self>) -> Result<VolumeInfo, OperationError> {
        Ok(VolumeInfo {
            name: self.name.to_ucstring(),
            serial_number: self.serial,
            fs_flags: winnt::FILE_READ_ONLY_VOLUME 
                | winnt::FILE_NAMED_STREAMS
                | winnt::FILE_UNICODE_ON_DISK,
            fs_name: U16CString::from_str("DieselFS").unwrap(),
            max_component_length: 255
        })
    }

    fn create_file(
        &self,
        _file_name: &U16CStr,
        _security_context: PDOKAN_IO_SECURITY_CONTEXT,
        _desired_access: winnt::ACCESS_MASK,
        _file_attributes: u32,
        _share_access: u32,
        _create_disposition: u32,
        _create_options: u32,
        _info: &mut OperationInfo<Self>
    ) -> Result<CreateFileInfo<Self::Context>, OperationError> {
        if (_desired_access & winnt::FILE_WRITE_DATA) != 0
            || (_desired_access & winnt::FILE_WRITE_ATTRIBUTES) != 0
            || (_desired_access & winnt::FILE_WRITE_EA) != 0
            || (_desired_access & winnt::FILE_APPEND_DATA) != 0
        {
            return Err(OperationError::NtStatus(ntstatus::STATUS_ACCESS_DENIED))
        }

        /*let path = _file_name.to_string_lossy();
        let filename_and_stream = path.rsplit('\\').next().unwrap();
        let si = filename_and_stream.rfind(":");
        let filename: &str;
        let stream: &str;
        match si {
            Some(idx) => {
                filename = &filename_and_stream[0..idx];
                stream = &filename_and_stream[(idx+1)..];
            }
            None => {
                filename = filename_and_stream;
                stream = "";
            }
        }*/
        
        let full_path = _file_name.to_string_lossy();
        let (path, stream) = split_stream_name(&full_path);
        println!("{:?} {:?}", path, stream);

        let inner_handle = self.fs.open_readable(path, stream)?;
        
        Ok(CreateFileInfo {
            is_dir: inner_handle.is_dir(),
            new_file_created: false,
            context: AdapterContext { handle: inner_handle }
        })
    }

    fn read_file (
        &self,
        _file_name: &U16CStr,
        _offset: i64,
        _buffer: &mut [u8],
        _info: &OperationInfo<Self>,
        _context: &Self::Context
    ) -> Result<u32, OperationError> {
        let readcount = _context.handle.read_at(_buffer, _offset.try_into().unwrap())?;
        Ok(readcount.try_into().unwrap())
    }

    fn find_files(
        &self,
        _file_name: &U16CStr,
        mut _fill_find_data: impl FnMut(&FindData) -> Result<(), FillDataError>,
        _info: &OperationInfo<Self>,
        _context: &Self::Context
    ) -> Result<(), OperationError> {
        let iter = _context.handle.find_files()?;
        for item in iter {
            _fill_find_data(&item)?;
        }
        Ok(())
    }

    fn find_streams(
        &self,
        _file_name: &U16CStr,
        mut _fill_find_stream_data: impl FnMut(&FindStreamData) -> Result<(), FillDataError>,
        _info: &OperationInfo<Self>,
        _context: &Self::Context
    ) -> Result<(), OperationError> {
        let iter = _context.handle.list_streams()?;
        for item in iter {
            _fill_find_stream_data(&item)?;
        }
        Ok(())
    }

    fn get_file_information(
        &self,
        _file_name: &U16CStr,
        _info: &OperationInfo<Self>,
        _context: &Self::Context
    ) -> Result<FileInfo, OperationError> {
        _context.handle.get_file_info()
    }
}

pub fn mount_test(mountpoint: &str) {
    let mp = U16CString::from_str(mountpoint).unwrap();
    let handler = DokanAdapter {
        fs: Arc::new(teststub::TestFs { }),
        name: U16CString::from_str("Test").unwrap(),
        serial: 0xf8be397b
    };

    Drive::new()
        .mount_point(&mp)
        .flags(MountFlags::ALT_STREAM | MountFlags::WRITE_PROTECT)
        .thread_count(0)
        .mount(&handler)
        .unwrap();
}

fn split_stream_name(full: &str) -> (&str, &str) {
    let lastbs = full.rfind('\\');
    match full.rfind(':') {
        None => (full, ""),
        Some(lastcolon) => {
            if lastcolon > lastbs.unwrap_or(0) {
                (&full[0..lastcolon], &full[(lastcolon+1)..])
            }
            else {
                (full, "")
            }
        }
    }
}