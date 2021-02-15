use std::fmt;
use std::ops::Range;

use fnv::FnvHashMap;

use super::diesel_hash;

#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Hash(pub u64);
impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:>016x}", &self.0)
    }
}

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

impl PartialEq for HashedStr<'_> {
    fn eq(&self, other: &HashedStr) -> bool {
        self.hash == other.hash
    }
}
impl Eq for HashedStr<'_> { }

impl PartialOrd for HashedStr<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        return Some(self.cmp(other))
    }
}

impl Ord for HashedStr<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.text {
            Some(st) => match other.text {
                Some(ot) => st.cmp(ot),
                None => std::cmp::Ordering::Less
            },
            None => match other.text {
                Some(_) => std::cmp::Ordering::Greater,
                None => self.hash.cmp(&other.hash)
            }
        }
    }
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
            result.index.insert(diesel_hash::hash_str(line), (line_start, line_start+line.len()));
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
            let r = &self.blobs[i].data[(indices.0)..(indices.1)];
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
            let r = &self.blobs[i].data[(indices.0)..(indices.1)];
            return HashedStr { hash, text: Some(r) };
        }

        let from_interned = self.interned.get(&hash);
        return HashedStr { hash, text: from_interned.map(String::as_str) }
    }

    pub fn get_str<'s>(&'s self, text: &str) -> HashedStr<'s> {
        self.get_hash(diesel_hash::hash_str(text))
    }

    /// Intern a string that's very likely a substring of one already loaded from a blob
    /// 
    /// If the parent string isn't in a blob, just intern normally. If not found at all,
    /// return None, otherwise return the substring's hash.
    pub fn intern_substring(&mut self, superstring_hash: u64, indices: Range<usize>) -> Option<u64> {
        for i in 0..self.blobs.len() {
            if !self.blobs[i].index.contains_key(&superstring_hash) {
                continue;
            }

            let superstring_indices = self.blobs[i].index.get(&superstring_hash).unwrap();
            let superstring = &self.blobs[i].data[(superstring_indices.0)..(superstring_indices.1)];
            let substring = &superstring[(indices.start)..(indices.end)];
            let substring_hash = diesel_hash::from_str(substring);

            let data_ptr = self.blobs[i].data.as_ptr() as usize;
            let substring_ptr = substring.as_ptr() as usize;
            let substring_start = substring_ptr.wrapping_sub(data_ptr);
            let substring_len = substring.len();
            self.blobs[i].index.insert(substring_hash, (substring_start, substring_start + substring_len));

            return Some(substring_hash);
        }

        let maybe_substring = self.interned.get(&superstring_hash).and_then(|superstring| {
            Some(superstring[(indices.start)..(indices.end)].to_owned())
        });

        match maybe_substring {
            None => return None,
            Some(substring) => {
                let hash = diesel_hash::hash_str(&substring);
                self.interned.insert(hash, substring);
                return Some(hash);
            }
        }
    }
}