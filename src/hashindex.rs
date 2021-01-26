use std::fmt;

use fnv::FnvHashMap;

use super::diesel_hash;

trait HashList {
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
        match &self.text {
            None => write!(f, "{:>016x}", &self.hash),
            Some(text) => if is_hash_like(&text) {
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

pub struct HashIndex {
    blobs: Vec<BlobHashIndex>,
    interned: FnvHashMap<u64, String>
}

impl HashIndex {
    pub fn new() -> HashIndex {
        let mut res = HashIndex {
            blobs: Vec::new(),
            interned: FnvHashMap::default()
        };
        res.interned.insert(diesel_hash::EMPTY, "".to_owned());
        return res;
    }

    pub fn load_blob(&mut self, data: String) {
        self.blobs.push(BlobHashIndex::new(data));
    }

    pub fn intern<'s>(&'s mut self, text: String) -> HashedStr<'s> {
        let hash = diesel_hash::hash_str(&text);
        for i in 0..self.blobs.len() {
            if !self.blobs[i].index.contains_key(&hash) {
                continue;
            }
            let indices = self.blobs[i].index.get(&hash).unwrap();
            let r = &self.blobs[i].data[(indices.0)..(indices.0 + indices.1)];
            return HashedStr { hash, text: Some(r) };
        }
        let e = self.interned.entry(hash);
        let et = e.or_insert(text);
        HashedStr { hash, text: Some(et)}
    }

    pub fn get_hash<'s>(&'s self, hash: u64) -> HashedStr<'s> {
        for i in 0..self.blobs.len() {
            if !self.blobs[i].index.contains_key(&hash) {
                continue;
            }
            let indices = self.blobs[i].index.get(&hash).unwrap();
            let r = &self.blobs[i].data[(indices.0)..(indices.0 + indices.1)];
            return HashedStr { hash, text: Some(r) };
        }

        let from_interned = self.interned.get(&hash);
        return HashedStr { hash, text: from_interned.map(String::as_str) }
    }

    pub fn get_str<'s>(&'s self, text: &str) -> HashedStr<'s> {
        self.get_hash(diesel_hash::hash_str(text))
    }
}