use std::collections::BTreeMap;

use crate::hashindex::{HashIndex, HashedStr};
use crate::diesel_hash;
use crate::util::*;

/* Diesel string tables have a header of:
    0..3   _           : [u8; 4]
    4..7   string_count: u32;
    8..31  _           : [u8; 24]
    32..?  entries     : [Entry; string_count]

  and Entry of:
    0..7   _           : [u8; 8]
    8..15  key_hash    : u64
    16..19 _           : [u8; 4]
    20..23 value_offset: u32

    The rest of the file is null-terminated utf-8 strings pointed to by Entry.value_offset
*/

pub fn map_from_bytes<'a>(hashlist: &'a HashIndex, bytes: &[u8]) -> BTreeMap<HashedStr<'a>, String> {
    let mut result = BTreeMap::<HashedStr, String>::new();

    let string_count = read_u32_le(bytes, 4);

    for i in 0..string_count {
        let entry_base : usize = 32 + (i as usize) * 24;
        let hash = read_u64_le(bytes, entry_base + 8);
        
        if hash == diesel_hash::EMPTY { continue; }
        
        let value_start = read_u32_le(bytes, entry_base + 20) as usize;
        let mut value_end = value_start;
        for i in value_start..(bytes.len()) {
            if bytes[i] != 0 { value_end += 1; }
            else { break; }
        }

        let value_bytes = &bytes[value_start..value_end];
        let value = String::from_utf8_lossy(value_bytes).into_owned();
        let key = hashlist.get_hash(hash);
        result.insert(key, value);
    }

    result
}

pub fn bytes_to_json<'a, O: std::io::Write>(hashlist: &'a HashIndex, input: &[u8], output: &mut O) -> std::io::Result<()> {
    let map = map_from_bytes(hashlist, input);
    output.write(b"{\n  ")?;
    let mut first = true;
    for (k,v) in map {
        if !first {
            output.write(b",\n  ")?;
        }
        first = false;
        let key_name = format!("{}", k);
        write!(output, "{}: {}", escape_json_str(&key_name), escape_json_str(&v))?;
    }
    output.write(b"}\n")?;
    Ok(())
}