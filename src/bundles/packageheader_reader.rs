use std::convert::TryInto;
use std::convert::TryFrom;
use std::collections::HashMap;
use crate::read_util::*;
use super::ReadError;

#[derive(Clone)]
pub struct PackageHeaderFile {
    pub entries: Vec<PackageHeaderEntry>
}

#[derive(Debug, Copy, Clone)]
pub struct PackageHeaderEntry {
    pub file_id: u32,
    pub offset: u32,
    pub length: u32
}

pub struct MultiBundleHeader {
    pub bundles: HashMap<u64, PackageHeaderFile>
}

pub fn read_normal(data: &[u8], datafile_length: u64) -> Result<PackageHeaderFile, ReadError> {
    let mut res = PackageHeaderFile {
        entries: Vec::new()
    };

    let is_x64 = read_int_le(data, 8) == 0 && read_int_le(data, 12) != 0;

    let ref_offset = read_int_le(data, 0);
    let item_count: u64;
    let offset: u64;
    let has_length: bool;

    if is_x64 {
        if read_long_le(data, 4) == read_long_le(data, 12) {
            item_count = read_long_le(data, 4);
            offset = read_long_le(data, 20);
            has_length = false;
        }
        else if read_long_le(data, 12) == read_long_le(data, 20) {
            item_count = read_long_le(data, 12);
            offset = read_int_le(data, 28).into();
            has_length = true;
        }
        else if read_long_le(data, 20) == read_int_le(data, 28).into() {
            item_count = read_long_le(data, 20);
            offset = read_int_le(data, 32).into();
            has_length = true;
        }
        else {
            return Err(ReadError::UnknownFormatOrMalformed);
        }
    }
    else {
        if read_int_le(data, 4) == read_int_le(data, 8) {
            item_count = read_int_le(data, 4).into();
            offset = read_int_le(data, 12).into();
            has_length = false;
        }
        else if read_int_le(data, 8) == read_int_le(data, 12) {
            item_count = read_int_le(data, 8).into();
            offset = read_int_le(data, 16).into();
            has_length = true;
        }
        else if read_int_le(data, 12) == read_int_le(data, 16) {
            item_count = read_int_le(data, 12).into();
            offset = read_int_le(data, 20).into();
            has_length = true;
        }
        else {
            return Err(ReadError::UnknownFormatOrMalformed);
        }
    }
    
    let actual_offset : usize = if offset == 0 {
        ref_offset.try_into().unwrap()
    } else {
        usize::try_from(offset).unwrap() + 4
    };

    if has_length {
        for i in 0..item_count {
            let offs : usize = actual_offset + usize::try_from(i).unwrap() * 12;
            res.entries.push(PackageHeaderEntry {
                file_id: read_int_le(data, offs+0),
                offset: read_int_le(data, offs+4),
                length: read_int_le(data, offs+8)
            });
        }
    }
    else {
        for i in 0..item_count {
            let offs : usize = actual_offset + usize::try_from(i).unwrap() * 8;
            let maybe_prev = res.entries.last_mut();
            let curr = PackageHeaderEntry {
                file_id: read_int_le(data, offs+0),
                offset: read_int_le(data, offs+4),
                length: 0
            };
            if let Some(prev) = maybe_prev {
                //println!("{:?} {:?}", prev, curr);
                prev.length = curr.offset - prev.offset;
            }
            res.entries.push(curr);
        }
        let maybe_prev = res.entries.last_mut();
        if let Some(prev) = maybe_prev {
            prev.length = u32::try_from(datafile_length).unwrap() - prev.offset;
        }
    }

    return Ok(res);
}

pub fn read_multi(data: &[u8]) -> Result<MultiBundleHeader, ReadError> {
    let mut res = MultiBundleHeader {
        bundles: HashMap::new()
    };
    
    let bundle_count = read_int_le(data, 4);
    let bundle_base: usize = 20;

    res.bundles.reserve(bundle_count.try_into().unwrap());

    for i in 0..bundle_count {
        let header_offs = bundle_base + 28 * (i as usize);
        let bundle_index = read_long_le(data, header_offs+0);
        let entry_count_1: usize = read_int_le(data, header_offs+8).try_into().unwrap();
        let entry_count_2: usize = read_int_le(data, header_offs+12).try_into().unwrap();
        let offset: usize = read_long_le(data, header_offs+16).try_into().unwrap();
        let always_one = read_int_le(data, header_offs+24);

        if always_one != 1 || entry_count_1 != entry_count_2 {
            return Err(ReadError::BadMultiBundleHeader);
        }

        let mut entries: Vec<PackageHeaderEntry> = Vec::new();
        entries.reserve_exact(entry_count_1);
        for ie in 0..entry_count_1 {
            let pe_offset = offset + (12*ie) + 4;
            let pe = PackageHeaderEntry {
                file_id: read_int_le(data, pe_offset+0),
                offset: read_int_le(data, pe_offset+4),
                length: read_int_le(data, pe_offset+8)
            };
            entries.push(pe);
        }
        res.bundles.insert(bundle_index, PackageHeaderFile { entries });
    }
    return Ok(res);
}