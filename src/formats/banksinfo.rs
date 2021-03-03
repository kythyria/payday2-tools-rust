use std::rc::Rc;
use fnv::FnvHashMap;

use crate::hashindex::Hash;
use crate::util::read_helpers::*;

#[derive(Debug)]
pub struct BanksInfo {
    pub sound_banks: Vec<Rc<str>>,
    pub sound_lookups: FnvHashMap<u64, (Hash, Rc<str>)>
}

#[derive(Debug)]
pub enum BankParseFailure {
    SliceError(TryFromBytesError),
    BadString(std::str::Utf8Error)
}
impl From<TryFromBytesError> for BankParseFailure { fn from(e: TryFromBytesError) -> BankParseFailure { BankParseFailure::SliceError(e) } }
impl From<std::str::Utf8Error> for BankParseFailure { fn from(e: std::str::Utf8Error) -> BankParseFailure { BankParseFailure::BadString(e) } }

pub fn try_from_bytes(src: &[u8]) -> Result<BanksInfo, BankParseFailure> {
    let bnk_count = u32::try_from_le(src, 0)? as usize;
    // skip a second copy of the count
    let bnk_offset = u32::try_from_le(src, 8)? as usize;
    let _section_pointer = u32::try_from_le(src, 12)?;
    let _unknown_1 = u32::try_from_le(src, 16)?;

    let sound_count = u32::try_from_le(src, 20)? as usize;
    // skip a second copy of the count
    let sound_offset = u32::try_from_le(src, 28)? as usize;
    let _section_pointer = u32::try_from_le(src, 32)?;
    let _unknown_2 = u32::try_from_le(src, 36)?;
    let _unknown_3 = u32::try_from_le(src, 40)?;

    let u_count = u32::try_from_le(src, 44)? as usize;
    // skip yet another copy of a count
    let u_offset = u32::try_from_le(src, 52)? as usize;

    let mut res = BanksInfo {
        sound_banks: Vec::with_capacity(bnk_count),
        sound_lookups: FnvHashMap::default()
    };
    res.sound_lookups.reserve(sound_count);
    
    for i in  0..bnk_count {
        let offset_offset = bnk_offset + i*8;

        // theres four zeroes skipped in each item, no idea what they're for.
        let start_offset = u32::try_from_le(src, offset_offset+4)? as usize;
        let mut end_offset = start_offset;
        while src[end_offset] != 0 { end_offset += 1; }
        let slice = &src[start_offset..end_offset];
        let text = std::str::from_utf8(slice)?;
        res.sound_banks.push(Rc::<str>::from(text));
    }

    let mut sound_hash_to_id = FnvHashMap::<Hash, u64>::default();
    sound_hash_to_id.reserve(sound_count);

    for i in 0..sound_count {
        let offset = sound_offset + i * 16;
        let id = u64::try_from_le(src, offset + 0)?;
        let hash = u64::try_from_le(src, offset + 8)?;
        sound_hash_to_id.entry(Hash(hash)).or_insert(id);
    }

    for i in 0..u_count {
        let offset = u_offset + i*16;
        let hash = Hash(u64::try_from_le(src, offset+0)?);
        let _zero = u32::try_from_le(src, offset+8)?;
        let string_offset = u32::try_from_le(src, offset+12)? as usize;
        let mut string_end = string_offset;
        while src[string_end] != 0 { string_end += 1; }
        let slice = &src[string_offset..string_end];
        let text = std::str::from_utf8(slice)?;
        let string = Rc::<str>::from(text);
        
        if let Some(id) = sound_hash_to_id.get(&hash) {
            res.sound_lookups.entry(*id).or_insert((hash, string));
        }
    }
    
    return Ok(res);
}