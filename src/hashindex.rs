use std::collections::HashMap;
use std::fmt;
use super::diesel_hash;

pub struct HashedStr<'a> {
    pub hash: u64,
    pub text: Option<&'a str>,
}

pub trait HashIndex {
    fn intern<'s>(&'s self, text: &str) -> HashedStr<'s>;
    fn get_hash<'s>(&'s self, hash: u64) -> HashedStr<'s>;
}

pub struct BlobHashIndex {
    index: HashMap<u64, (usize, usize)>,
    data: String,
}

impl BlobHashIndex {
    pub fn new(data: String) -> BlobHashIndex {
        let mut result = BlobHashIndex {
            data,
            index : HashMap::new()
        };
        let data_start = result.data.as_ptr() as usize;
        for line in result.data.lines() {
            let line_start_ptr = line.as_ptr() as usize;
            let line_start = line_start_ptr.wrapping_sub(data_start);
            result.index.insert(diesel_hash::hash_str(line), (line_start, line.len()));
        }
        return result;
    }
}

impl HashIndex for BlobHashIndex {
    fn intern(&self, text: &str) -> HashedStr {
        let hash = diesel_hash::hash_str(text);
        return self.get_hash(hash);
    }

    fn get_hash(&self, hash: u64) -> HashedStr {
        let res = self.index.get(&hash);
        match res {
            None => HashedStr { hash, text: None },
            Some((start, len)) => HashedStr { hash, text: Some(&self.data[*start..(*start+*len)])}
        }
    }
}

impl fmt::Display for HashedStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:>016x}, {:?})", &self.hash, &self.text)
    }
}