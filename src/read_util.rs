use std::convert::TryInto;

pub fn read_int_le(src: &[u8], idx: usize) -> u32 {
    return u32::from_le_bytes(src[idx..(idx+4)].try_into().unwrap());
}

pub fn read_long_le(src: &[u8], idx: usize) -> u64 {
    return u64::from_le_bytes(src[idx..(idx+8)].try_into().unwrap());
}