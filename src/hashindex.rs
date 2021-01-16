use fnv::FnvHashMap;
use std::fmt;
use super::diesel_hash;

pub trait HashIndex {
    fn intern<'s>(&'s mut self, text: &str) -> HashedStr<'s>;
    fn get_hash<'s>(&'s self, hash: u64) -> HashedStr<'s>;
    fn get_str<'s>(&'s self, text: &str) -> HashedStr<'s> {
        self.get_hash(diesel_hash::hash_str(text))
    }
}

pub struct HashedStr<'a> {
    pub hash: u64,
    pub text: Option<&'a str>,
}

impl fmt::Debug for HashedStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:>016x}, {:?})", &self.hash, &self.text)
    }
}

impl fmt::Display for HashedStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.text {
            None => write!(f, "{:>016x}", &self.hash),
            Some(text) => if is_hash_like(text) {
                    write!(f, "{:>016x}", &self.hash)
                }
                else {
                    write!(f, "{}", text)
                }
        }
    }
}

fn is_hash_like(txt: &str) -> bool {
    if txt.len() != 16 { return false; }
    for i in txt.chars() {
        if ('0'..'9').contains(&i) || ('a'..'z').contains(&i) || ('A'..'Z').contains(&i) {
            continue;
        }
        return false;
    }
    return true;
}

pub struct MapHashIndex {
    data: FnvHashMap<u64, String>
}

impl MapHashIndex {
    pub fn new() -> MapHashIndex {
        MapHashIndex { data: FnvHashMap::default() }
    }
}

impl HashIndex for MapHashIndex {
    fn intern(&mut self, text: &str) -> HashedStr {
        let hash = diesel_hash::hash_str(text);
        if !self.data.contains_key(&hash) {
            self.data.insert(hash, String::from(text));
        }
        let already = self.data.get(&hash).unwrap();
        HashedStr { hash, text: Some(already) }
    }

    fn get_hash(&self, hash: u64) -> HashedStr {
        let v = self.data.get(&hash);
        match v {
            Some(text) => HashedStr { hash, text: Some(text) },
            None => HashedStr { hash, text: None }
        }
    }
}

pub struct BlobHashIndex {
    index: FnvHashMap<u64, (usize, usize)>,
    data: String,
}

impl BlobHashIndex {
    pub fn new(data: String) -> BlobHashIndex {
        let mut result = BlobHashIndex {
            data,
            index : FnvHashMap::default()
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
    fn intern(&mut self, text: &str) -> HashedStr {
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