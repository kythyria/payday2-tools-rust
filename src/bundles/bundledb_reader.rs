use std::convert::TryInto;

pub struct LanguageEntry {
    pub hash: u64,
    pub id: u32
}

pub struct FileEntry {
    pub path: u64,
    pub extension: u64,
    pub file_id: u32,
    pub lang_id: u32,
}

pub struct BundleDbFile {
    pub tag: u32,
    pub is_raid : bool,
    pub is_x64 : bool,
    pub languages : Vec<LanguageEntry>,
    pub files: Vec<FileEntry>
}

fn read_int_le(src: &[u8], idx: usize) -> u32 {
    return u32::from_le_bytes(src[idx..(idx+4)].try_into().unwrap());
}

fn read_long_le(src: &[u8], idx: usize) -> u64 {
    return u64::from_le_bytes(src[idx..(idx+8)].try_into().unwrap());
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
        is_raid: false,
        is_x64: false,
        languages: std::vec::Vec::new(),
        files: std::vec::Vec::new()
    };

    res.tag = read_int_le(blb, 0);
    let maybe_lang_count = read_int_le(blb, 4);
    if maybe_lang_count != 0 { // PD2
        let lang_count = maybe_lang_count;
        let lang_offset = read_int_le(blb, 12);
        let file_entries_count = read_int_le(blb, 28);
        let file_entries_offset = read_int_le(blb, 36);
        read_bdb_languages(&blb, lang_offset.into(), lang_count.into(), &mut res.languages);
        read_bdb_files(blb, file_entries_offset.into(), file_entries_count.into(), &mut res.files);
    }
    else { // x64 and raid
        let lang_count = read_int_le(blb, 8);
        let lang_offset : u64;
        let file_entries_count: u32;
        let file_entries_offset: u64;
        let discriminator = read_int_le(blb, 12);
        if discriminator != 0 { //x64
            lang_offset = read_long_le(blb, 16);
            file_entries_count = read_int_le(blb, 48);
            file_entries_offset = read_long_le(blb, 56);
        }
        else { //raid
            lang_offset = read_long_le(blb,24);
            file_entries_count = read_int_le(blb, 56);
            file_entries_offset = read_long_le(blb, 72);
        }
        read_bdb_languages(&blb, lang_offset, lang_count.into(), &mut res.languages);
        read_bdb_files(blb, file_entries_offset.into(), file_entries_count.into(), &mut res.files);
    }

    return res;
}

fn read_bdb_languages(blb: &[u8], offset: u64, count: u64, dest: &mut Vec<LanguageEntry>) {
    dest.reserve(count.try_into().unwrap());
    for i in 0..count {
        let entry_offset : usize = (offset + i*16).try_into().unwrap();
        let le = LanguageEntry {
            hash: read_long_le(blb, entry_offset+0),
            id: read_int_le(blb, entry_offset+8)
        };
        dest.push(le);
    }
}

fn read_bdb_files(blb: &[u8], offset: u64, count: u64, dest: &mut Vec<FileEntry>) {
    dest.reserve(count.try_into().unwrap());
    for i in 0..count {
        let entry_offset : usize = (offset + i*32).try_into().unwrap();
        let fe = FileEntry {
            extension: read_long_le(blb, entry_offset+0),
            path: read_long_le(blb, entry_offset+8),
            lang_id: read_int_le(blb, entry_offset+16),
            file_id: read_int_le(blb, entry_offset+24)
        };
        dest.push(fe);
    }
}