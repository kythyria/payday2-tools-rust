use std::convert::TryInto;

#[derive(Debug)]
pub struct TryFromBytesError { pub idx: usize }
impl std::fmt::Display for TryFromBytesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to get integer from bytes at position {}", self.idx)
    }
}
impl std::error::Error for TryFromBytesError {}

macro_rules! read_le {
    ($($name:ident : $len:expr => $type:ident;)*) => {
        $(pub fn $name(src: &[u8], idx: usize) -> $type {
            return $type::from_le_bytes(src[idx..(idx+$len)].try_into().unwrap());
        }

        impl TryFromIndexedLE for $type {
            type Error = TryFromBytesError;
            fn try_from_le(src: &[u8], idx: usize) -> Result<Self, Self::Error> {
                let slice = src.get(idx..(idx+$len)).ok_or(TryFromBytesError{idx})?;
                Ok($type::from_le_bytes(slice.try_into().or(Err(TryFromBytesError{idx}))?))
            }
        })*
    }
}

pub trait TryFromIndexedLE: Sized {
    type Error;
    fn try_from_le(src: &[u8], index: usize) -> Result<Self, Self::Error>;
}

read_le! {
    read_u16_le: 2 => u16;
    read_u32_le: 4 => u32;
    read_u64_le: 8 => u64;

    read_i16_le: 2 => i16;
    read_i32_le: 4 => i32;
    read_i64_le: 8 => i64;

    read_f32_le: 4 => f32;
    read_f64_le: 8 => f64;
}

