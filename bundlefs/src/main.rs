use std::convert::TryInto;
use std::sync::Arc;

use dokan::*;
use structopt::StructOpt;
use widestring::{U16CString, U16CStr};
use winapi::shared::ntstatus;
use winapi::um::winnt;

use pd2tools_rust::bundles::database::Database;
use pd2tools_rust::filesystem::{raw_bundledb, transcoder, teststub};
use pd2tools_rust::filesystem::{FsError, FsReadHandle, ReadOnlyFs};
use pd2tools_rust::util::LIB_VERSION;

#[derive(Debug, StructOpt)]
#[structopt(name="Payday 2 BundleFS", about="Mount asset bundles from Payday 2 as a Dokany filesystem", version=LIB_VERSION)]
struct Opt {
    /// Path of hashlist to use. By default look in cwd and then next to the executable.
    #[structopt(short, long)]
    hashlist: Option<String>,

    /// Directory containing bundle_db.blb
    asset_dir: String,
    /// Drive letter to mount on
    mountpoint: String
}

fn main() {
    let opt = Opt::from_args();

    let hashlist = pd2tools_rust::get_hashlist(&opt.hashlist).unwrap();
    let db = pd2tools_rust::get_packagedb(hashlist, &opt.asset_dir).unwrap();
    mount_cooked_database(&opt.mountpoint, db.hashes.clone(), Arc::new(db));
}

pub fn mount_cooked_database(mountpoint: &str, hashlist: Arc<pd2tools_rust::hashindex::HashIndex>, db: Arc<Database>) {
    let mp = U16CString::from_str(mountpoint).unwrap();
    let rawdb : Arc<dyn ReadOnlyFs> = Arc::new(raw_bundledb::BundleFs::new(db));
    let handler = DokanAdapter {
        fs: transcoder::TranscoderFs::new(hashlist, rawdb),
        name: U16CString::from_str("Diesel Assets").unwrap(),
        serial: 0xf8be397b
    };
    
    {
        let mut drive = Drive::new();
        drive
            .mount_point(&mp)
            .flags(MountFlags::ALT_STREAM | MountFlags::WRITE_PROTECT)
            .thread_count(0)
            .mount(&handler)
            .unwrap();
    }
    ()
}

pub fn mount_test(mountpoint: &str) {
    let mp = U16CString::from_str(mountpoint).unwrap();
    let handler = DokanAdapter {
        fs: teststub::TestFs { },
        name: U16CString::from_str("Test").unwrap(),
        serial: 0xf8be397b
    };
    
    {
        let mut drive = Drive::new();
        drive
            .mount_point(&mp)
            .flags(MountFlags::ALT_STREAM | MountFlags::WRITE_PROTECT)
            .thread_count(0)
            .mount(&handler)
            .unwrap();
    }
    ()
}

pub fn mount_raw_database(mountpoint: &str, db: Arc<Database>) {
    let mp = U16CString::from_str(mountpoint).unwrap();
    let handler = DokanAdapter {
        fs: raw_bundledb::BundleFs::new(db),
        name: U16CString::from_str("Diesel Assets").unwrap(),
        serial: 0xf8be397b
    };
    
    {
        let mut drive = Drive::new();
        drive
            .mount_point(&mp)
            .flags(MountFlags::ALT_STREAM | MountFlags::WRITE_PROTECT)
            .thread_count(0)
            .mount(&handler)
            .unwrap();
    }
    ()
}

struct DokanAdapter<F: ReadOnlyFs> {
    fs: F,
    serial: u32,
    name: U16CString,
    //_phantom: PhantomData<&'fs &'ctx ()>
}

pub struct AdapterContext<'a> {
    handle: Arc<dyn FsReadHandle + 'a>
}

impl<'ctx, 'fs: 'ctx, F: ReadOnlyFs + 'fs + 'ctx> FileSystemHandler<'ctx, 'fs> for DokanAdapter<F> {
    type Context = AdapterContext<'ctx>;

    fn get_volume_information(&'fs self, _info: &OperationInfo<'ctx, 'fs, Self>) -> Result<VolumeInfo, OperationError> {
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
        &'fs self,
        _file_name: &U16CStr,
        _security_context: PDOKAN_IO_SECURITY_CONTEXT,
        _desired_access: winnt::ACCESS_MASK,
        _file_attributes: u32,
        _share_access: u32,
        _create_disposition: u32,
        _create_options: u32,
        _info: &mut OperationInfo<'ctx, 'fs, Self>
    ) -> Result<CreateFileInfo<Self::Context>, OperationError> {
        if (_desired_access & winnt::FILE_WRITE_DATA) != 0
            || (_desired_access & winnt::FILE_WRITE_ATTRIBUTES) != 0
            || (_desired_access & winnt::FILE_WRITE_EA) != 0
            || (_desired_access & winnt::FILE_APPEND_DATA) != 0
        {
            return Err(OperationError::NtStatus(ntstatus::STATUS_ACCESS_DENIED))
        }
        
        let full_path = _file_name.to_string_lossy();
        let (path, stream) = split_stream_name(&full_path);

        let inner_handle = self.fs.open_readable(path, stream).into2()?;
        
        Ok(CreateFileInfo {
            is_dir: inner_handle.is_dir(),
            new_file_created: false,
            context: AdapterContext { handle: inner_handle }
        })
    }

    fn read_file (
        &'fs self,
        _file_name: &U16CStr,
        _offset: i64,
        _buffer: &mut [u8],
        _info: &OperationInfo<'ctx, 'fs, Self>,
        _context: &Self::Context
    ) -> Result<u32, OperationError> {
        let readcount = _context.handle.read_at(_buffer, _offset.try_into().unwrap()).into2()?;
        Ok(readcount.try_into().unwrap())
    }

    fn find_files(
        &'fs self,
        _file_name: &U16CStr,
        mut _fill_find_data: impl FnMut(&FindData) -> Result<(), FillDataError>,
        _info: &OperationInfo<'ctx, 'fs, Self>,
        _context: &Self::Context
    ) -> Result<(), OperationError> {
        let iter = _context.handle.find_files().into2()?;
        for item in iter {
            _fill_find_data(&FindData {
                file_name: U16CString::from_str(&item.name).unwrap(),
                attributes: winnt::FILE_ATTRIBUTE_READONLY | if item.is_dir { winnt::FILE_ATTRIBUTE_DIRECTORY } else { 0 },
                file_size: item.size,
                creation_time: item.modification_time,
                last_access_time: item.modification_time,
                last_write_time: item.modification_time
            })?;
        }
        Ok(())
    }

    fn find_streams(
        &'fs self,
        _file_name: &U16CStr,
        mut _fill_find_stream_data: impl FnMut(&FindStreamData) -> Result<(), FillDataError>,
        _info: &OperationInfo<'ctx, 'fs, Self>,
        _context: &Self::Context
    ) -> Result<(), OperationError> {
        let iter = _context.handle.list_streams().into2()?;
        for item in iter {
            let fsd = FindStreamData {
                size: item.size,
                name: U16CString::from_str(format!(":{}:$DATA", item.name)).unwrap()
            };
            _fill_find_stream_data(&fsd)?;
        }
        Ok(())
    }

    fn get_file_information(
        &'fs self,
        _file_name: &U16CStr,
        _info: &OperationInfo<'ctx, 'fs, Self>,
        _context: &Self::Context
    ) -> Result<FileInfo, OperationError> {
        let fi = _context.handle.get_file_info().into2()?;
        let mut att = 0;
        if fi.is_dir {
            att |= winnt::FILE_ATTRIBUTE_DIRECTORY;
        }
        if fi.read_only {
            att |= winnt::FILE_ATTRIBUTE_READONLY;
        }
        Ok(FileInfo {
            creation_time: fi.creation_time,
            last_access_time: fi.last_access_time,
            last_write_time: fi.last_write_time,
            file_index: fi.file_index,
            file_size: fi.file_size,
            number_of_links: fi.number_of_links,
            attributes: att
        })
    }
}

trait IntoExt<T> {
    fn into2(self) -> T;
}
impl<T> IntoExt<Result<T, OperationError>> for Result<T, FsError> {
    fn into2(self) -> Result<T, OperationError> {
        self.map_err(|e| <FsError as IntoExt<OperationError>>::into2(e))
    }
}
impl IntoExt<OperationError> for FsError {
    fn into2(self) -> OperationError {
        match self {
            FsError::PastEnd => OperationError::NtStatus(ntstatus::STATUS_BEYOND_VDL),
            FsError::FileCorrupt => OperationError::NtStatus(ntstatus::STATUS_FILE_CORRUPT_ERROR),
            FsError::NotDirectory => OperationError::NtStatus(ntstatus::STATUS_NOT_A_DIRECTORY),
            FsError::IsDirectory => OperationError::NtStatus(ntstatus::STATUS_FILE_IS_A_DIRECTORY),
            FsError::NotFound => OperationError::NtStatus(ntstatus::STATUS_NOT_FOUND),
            FsError::ReadError => OperationError::Win32(winapi::shared::winerror::ERROR_READ_FAULT),
            FsError::OsError(oe) => OperationError::Win32(oe.try_into().unwrap())
        }
    }
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