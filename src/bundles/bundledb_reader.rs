use std::convert::TryInto;

use pd2tools_macros::Parse;
use crate::util::read_helpers::*;
use crate::util::parse_helpers;
use crate::util::parse_helpers::Parse;

#[derive(Parse)]
pub struct LanguageEntry {
    pub hash: u64,
    pub id: u32
}

#[derive(Parse)]
pub struct FileEntry {
    pub extension: u64,
    pub path: u64,
    pub lang_id: u32,
    #[skip_before(4)] pub file_id: u32,
}

pub struct BundleDbFile {
    pub tag: u32,
    pub languages : Vec<LanguageEntry>,
    pub files: Vec<FileEntry>
}

/* there's three possible layouts for the bdb header

PD2 form {
    tag: u32,
    lang_count: u32,
    _: pad[4],
    lang_offset: u32,
    _: pad[12],
    file_entries_count: u32,
    _: pad[4],
    file_entries_offset: u32
}

x64 form {
    tag: u32,
    _: zero[4],
    lang_count: u32,
    _: nonzero[4],
    lang_offset: u64,
    _: pad[24],
    file_entries_count: u32,
    _: pad[4],
    file_entries_offset: u64
}

raid form {
    tag: u32,
    _: zero[4],
    lang_count: u32,
    _: zero[4],
    _: pad[8],
    lang_offset: u64,
    _: pad[24],
    file_entries_count: u32,
    _: pad[12],
    file_entries_offset: u64
}

*/

pub fn read_bundle_db(blb: &[u8]) -> BundleDbFile {
    let mut res = BundleDbFile {
        tag: 0,
        languages: std::vec::Vec::new(),
        files: std::vec::Vec::new()
    };

    res.tag = read_u32_le(blb, 0);
    let maybe_lang_count = read_u32_le(blb, 4);
    let lang_count : u32;
    let lang_offset : u64;
    let file_entries_count : u32;
    let file_entries_offset: u64;
    if maybe_lang_count != 0 { // PD2
        lang_count = maybe_lang_count;
        lang_offset = read_u32_le(blb, 12).into();
        file_entries_count = read_u32_le(blb, 28);
        file_entries_offset = read_u32_le(blb, 36).into();
    }
    else { // x64 and raid
        lang_count = read_u32_le(blb, 8);
        let discriminator = read_u32_le(blb, 12);
        if discriminator != 0 { //x64
            lang_offset = read_u64_le(blb, 16);
            file_entries_count = read_u32_le(blb, 48);
            file_entries_offset = read_u64_le(blb, 56);
        }
        else { //raid
            lang_offset = read_u64_le(blb,24);
            file_entries_count = read_u32_le(blb, 56);
            file_entries_offset = read_u64_le(blb, 72);
        }
    }

    res.languages = parse_array_strided_unwrap(&blb[(lang_offset as usize)..], lang_count as usize, 16);
    res.files = parse_array_strided_unwrap(&blb[(file_entries_offset as usize)..], file_entries_count as usize, 32);

    return res;
}

fn parse_array_strided_unwrap<T: Parse>(data: &[u8], count: usize, stride: usize) -> Vec<T> {
    let mut dest = Vec::<T>::with_capacity(count);
    for i in 0..count {
        let offset = i*stride;
        let slice = &data[offset..(offset+stride)];
        let (_, entry) = <T as Parse>::parse(slice).unwrap();
        dest.push(entry);
    }
    return dest;
}