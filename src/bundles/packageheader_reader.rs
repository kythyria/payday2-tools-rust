use std::convert::TryInto;
use std::convert::TryFrom;
use crate::read_util::*;
use super::ReadError;

pub struct PackageHeaderFile {
    pub entries: Vec<PackageHeaderEntry>
}

pub struct PackageHeaderEntry {
    pub file_id: u32,
    pub offset: u32,
    pub length: u32
}

pub fn read_header_nonmulti(data: &[u8], datafile_length: u64) -> Result<PackageHeaderFile, ReadError> {
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
            return Err(ReadError::UnknownFormat);
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
            return Err(ReadError::UnknownFormat);
        }
    }

    let actual_offset : usize = if offset == 0 {
        ref_offset.try_into().unwrap()
    } else {
        offset.try_into().unwrap()
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
            let offs : usize = usize::try_from(i).unwrap() * 8;
            let maybe_prev = res.entries.last_mut();
            if let Some(prev) = maybe_prev {
                prev.length = u32::try_from(offs).unwrap() - prev.offset;
            }
            let curr = PackageHeaderEntry {
                file_id: read_int_le(data, offs+0),
                offset: read_int_le(data, offs+4),
                length: 0
            };
            res.entries.push(curr);
        }
        let maybe_prev = res.entries.last_mut();
        if let Some(prev) = maybe_prev {
            prev.length = u32::try_from(datafile_length).unwrap() - prev.offset;
        }
    }

    return Ok(res);
}