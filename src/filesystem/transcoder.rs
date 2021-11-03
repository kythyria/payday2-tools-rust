use std::convert::TryInto;
use std::io::Read;
use std::sync::Arc;
use std::time::SystemTime;

use crate::hashindex::HashIndex;
use super::{ReadOnlyFs, FsReadHandle, FsDirEntry, FsError, FsFileInfo, FsStreamEntry};

pub struct TranscoderFs<'a> {
    hashlist: Arc<HashIndex>,
    backing: Arc<dyn ReadOnlyFs + 'a>
}

impl<'a> TranscoderFs<'a> {
    pub fn new(hashlist: Arc<HashIndex>, backing: Arc<dyn ReadOnlyFs + 'a>) -> TranscoderFs<'a> {
        TranscoderFs {
            hashlist,
            backing
        }
    }
}

impl ReadOnlyFs for TranscoderFs<'_> {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, FsError> {
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
    fn get_file_info(&self) -> Result<FsFileInfo, FsError> {
        self.backing.get_file_info()
    }
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, FsError> {
        self.backing.read_at(buf, offset)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        self.backing.list_streams()
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
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
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, FsError> {
        let ofs: usize = offset.try_into().unwrap_or(usize::MAX);
        if ofs > self.data.len() {
            return Err(FsError::PastEnd)
        }
        let mut bs = &self.data[ofs..];
        bs.read(buf).or(Err(FsError::FileCorrupt))
    }
    fn find_files(&self) -> Result<Box<dyn Iterator<Item=FsDirEntry>>, FsError> {
        Err(FsError::NotDirectory)
    }
    fn list_streams(&self) -> Result<Box<dyn Iterator<Item=FsStreamEntry>>, FsError> {
        Ok(Box::new(std::iter::once(
            FsStreamEntry {
                name: String::from("raw"),
                size: self.data.len() as i64
            }
        )))
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
        // renames
        (".movie"           , ".bik"             , true , None                       ),
        (".texture"         , ".dds"             , true , None                       ),
        (".stream"          , ".wem"             , true , None                       ),

        // non-scriptdata
        (".strings"         , ".strings"         , true , Some(transcode_strings   ) ),
        (".banksinfo"       , ".banksinfo"       , true , Some(transcode_banksinfo ) ),
        
        // specific scriptdata files
        ("mission.mission"  , "mission.mission"  , true , Some(transcode_sd_custom ) ),
        ("world.world"      , "world.world"      , true , Some(transcode_sd_generic) ),

        // extensions
        (".achievement"     , ".achievement"     , true , Some(transcode_sd_custom ) ),
        (".action_message"  , ".action_message"  , true , Some(transcode_sd_custom ) ),
        (".credits"         , ".credits"         , true , Some(transcode_sd_custom ) ),
        (".comment"         , ".comment"         , true , Some(transcode_sd_custom ) ),
        (".continent"       , ".continent"       , true , Some(transcode_sd_custom ) ),
        (".continents"      , ".continents"      , true , Some(transcode_sd_custom ) ),
        (".cover_data"      , ".cover_data"      , true , Some(transcode_sd_generic) ),
        (".dialog"          , ".dialog"          , true , Some(transcode_sd_custom ) ),
        (".environment"     , ".environment"     , true , Some(transcode_sd_custom ) ),
        (".hint"            , ".hint"            , true , Some(transcode_sd_custom ) ),
        (".menu"            , ".menu"            , true , Some(transcode_sd_custom ) ),
        (".mission"         , ".mission"         , true , Some(transcode_sd_generic) ),
        (".nav_data"        , ".nav_data"        , true , Some(transcode_sd_generic) ),
        (".objective"       , ".objective"       , true , Some(transcode_sd_custom ) ),
        (".sequence_manager", ".sequence_manager", true , Some(transcode_sd_generic) ),
        (".timeline"        , ".timeline"        , true , Some(transcode_sd_custom ) ),
        (".world"           , ".world"           , true , Some(transcode_sd_generic) ),
        (".world_cameras"   , ".world_cameras"   , true , Some(transcode_sd_custom ) ),
        (".world_sounds"    , ".world_sounds"    , true , Some(transcode_sd_generic) )
    ] 
}

fn transcode_strings(hi: &HashIndex, input: &[u8]) -> Vec<u8> {
    let mut buf = Vec::<u8>::with_capacity(input.len());
    crate::formats::string_table::bytes_to_json(hi, input, &mut buf).unwrap();
    buf
}

fn transcode_sd_generic(_hi: &HashIndex, input: &[u8]) -> Vec<u8> {
    let doc = crate::formats::scriptdata::binary::from_binary(input, false);
    let gx = crate::formats::scriptdata::generic_xml::dump(&doc.unwrap());
    return gx.into_bytes();
}

fn transcode_sd_custom(_hi: &HashIndex, input: &[u8]) -> Vec<u8> {
    let doc = crate::formats::scriptdata::binary::from_binary(input, false);
    let gx = crate::formats::scriptdata::custom_xml::dump(&doc.unwrap());
    return gx.into_bytes();
}

fn transcode_banksinfo(_hi: &HashIndex, input: &[u8]) -> Vec<u8> {
    let bkif = crate::formats::banksinfo::try_from_bytes(input);
    let s = format!("{:?}", bkif);
    return s.into_bytes();
}