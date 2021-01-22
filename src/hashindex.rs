use std::fmt;
use std::cell::{RefCell, Ref};
use std::rc::Rc;

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
    pub text: Option<Ref<'a,str>>,
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
    stuff: Rc<RefCell<HashIndexData>>
}

impl HashIndex {
    pub fn new() -> HashIndex {
        let rc = Rc::new(RefCell::new(HashIndexData {
            blobs: Vec::new(),
            interned: FnvHashMap::default()
        }));
        HashIndex {
            stuff: rc
        }
    }

    pub fn load_blob(&self, data: String) {
        let mut s = self.stuff.borrow_mut();
        (*s).blobs.push(BlobHashIndex::new(data));
    }

    pub fn intern<'s>(&'s self, text: &str) -> HashedStr<'s> {
        let existing = self.get_str(text);
        let hash = existing.hash;
        match existing.text {
            Some(_) => existing,
            None => {
                let mut s = self.stuff.borrow_mut();
                s.interned.insert(hash, text.to_owned());
                self.get_hash(hash)
            }
        }
    }

    pub fn get_hash<'s>(&'s self, hash: u64) -> HashedStr<'s> {
        let s = self.stuff.borrow();
        for i in 0..(*s).blobs.len() {
            if !s.blobs[i].index.contains_key(&hash) {
                continue;
            }
            let indices = *s.blobs[i].index.get(&hash).unwrap();
            let r = Ref::map(s, |t| &t.blobs[i].data[(indices.0)..(indices.1)]);
            return HashedStr { hash, text: Some(r) };
        }

        if (*s).interned.contains_key(&hash) {
            return HashedStr { hash, text: Some(Ref::map(s, |t| t.interned.get(&hash).unwrap().as_str())) }
        }
        else {
            return HashedStr { hash, text: None };
        }
    }

    fn get_str<'s>(&'s self, text: &str) -> HashedStr<'s> {
        self.get_hash(diesel_hash::hash_str(text))
    }
}

struct HashIndexData {
    pub blobs: Vec<BlobHashIndex>,
    pub interned: FnvHashMap<u64, String>
}