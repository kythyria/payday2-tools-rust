use std::{fs::File, iter::FromIterator, path::Path};
use std::io;
use std::os::windows::fs::FileExt;
use std::rc::Rc;
use fnv::FnvHashSet;

use crate::bundles::database::{Database, ReadItem};
use crate::diesel_hash::{hash_str as dhash};

mod scriptdata;
use scriptdata::*;

pub fn do_scan<W: std::io::Write>(db: &Database, output: &mut W) -> io::Result<()> {
    let to_read = db.filter_key_sort_physical(|key| {
        key.extension.hash == dhash("credits")
        || key.extension.hash == dhash("dialog_index")
        || key.extension.hash == dhash("sequence_manager")
        || key.extension.hash == dhash("continent")
        || (key.extension.hash == dhash("continents") && key.path.text.is_some())
        || (key.extension.hash == dhash("world") && key.path.text.is_some())
        || key.extension.hash == dhash("mission")
    });

    let mut found = do_scan_pass(to_read)?;

    let mut ordered: Vec<Rc<str>> = Vec::from_iter(found.drain());
    ordered.sort();
    for s in &ordered {
        writeln!(output, "{}", s)?;
    }
    Ok(())
}

fn do_scan_pass(to_read: Vec<(&Path, Vec<ReadItem>)>) -> io::Result<FnvHashSet<Rc<str>>> {
    let mut found = FnvHashSet::<Rc<str>>::default();

    for (path, items) in to_read {
        let bundle = File::open(path)?;
        for item in items {
            let mut bytes = Vec::<u8>::with_capacity(item.length);
            bundle.seek_read(&mut bytes, item.offset as u64)?;
            let scanned = do_scan_buffer(bytes, item);
            match scanned {
                Err(e) => eprintln!("Failed reading \"{:?}\": {:?}", item.key, e),
                Ok(Some(v)) => found.extend(v),
                _ => ()
            }
        }
    }
    return Ok(found);
}

fn do_scan_buffer(buf: Vec<u8>, item: ReadItem) -> Result<Option<Vec<Rc<str>>>,()>{
    let doc = crate::formats::scriptdata::binary::from_binary(&buf, false);
    let iter = match item.key.extension.text {
        Some("credits") => scan_credits(&doc),
        Some("dialog_index") => scan_dialog_index(&doc),
        Some("sequence_manager") => scan_sequence_manager(&doc),
        Some("continent") => scan_continent(&doc),
        Some("continents") => scan_continents(&doc, Rc::from(item.key.path.text.unwrap())),
        Some("world") => scan_world(&doc, Rc::from(item.key.path.text.unwrap())),
        Some("mission") => scan_mission(&doc),
        _ => return Ok(None)
    };
    let result = iter.collect::<Vec<_>>();
    return Ok(Some(result));
}

