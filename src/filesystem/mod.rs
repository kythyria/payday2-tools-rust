use dokan::*;
use widestring::{U16CString, U16CStr};
use winapi::um::winnt;

pub struct RootFsHandler {
    
}

pub struct RootFsContext {

}

trait FsContext {
    fn is_dir(&self) -> bool;
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, OperationError>;
    fn find_files(&self, receiver: impl FnMut(&FindData) -> Result<(), FillDataError>) -> Result<(), OperationError>;
    fn list_streams(&self, receiver: impl FnMut(&FindStreamData) -> Result<(), FillDataError>) -> Result<(), OperationError>;
}

impl<'c, 's: 'c> FileSystemHandler<'c, 's> for RootFsHandler {
    type Context = RootFsContext;

    fn get_volume_information(&self, _info: &OperationInfo<Self>) -> Result<VolumeInfo, OperationError> {
        Ok(VolumeInfo {
            name: U16CString::from_str("Diesel Bundles").unwrap(),
            serial_number: 0xf8be397b, // TODO: Sensible serial number.
            fs_flags: winnt::FILE_READ_ONLY_VOLUME 
                | winnt::FILE_NAMED_STREAMS
                | winnt::FILE_UNICODE_ON_DISK,
            fs_name: U16CString::from_str("NTFS").unwrap(),
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
        unimplemented!()
    }
}