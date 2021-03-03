use std::{fs::File, iter::FromIterator, path::Path};
use std::io;
use std::os::windows::fs::FileExt;
use std::rc::Rc;
use fnv::FnvHashSet;

use crate::bundles::database::{Database, ReadItem};
use crate::diesel_hash::{hash_str as dhash};

mod scriptdata;

mod xml;

pub fn do_scan<W: std::io::Write>(db: &Database, output: &mut W) -> io::Result<()> {
    let to_read = db.filter_key_sort_physical(|key| {
        key.extension.hash == dhash("credits")
        || key.extension.hash == dhash("dialog_index")
        || key.extension.hash == dhash("sequence_manager")
        || key.extension.hash == dhash("continent")
        || (key.extension.hash == dhash("continents") && key.path.text.is_some())
        || (key.extension.hash == dhash("world") && key.path.text.is_some())
        || key.extension.hash == dhash("mission")
        || key.extension.hash == dhash("object")
        || key.extension.hash == dhash("animation_state_machine")
        || key.extension.hash == dhash("animation_subset")
        || key.extension.hash == dhash("effect")
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
            let mut bytes = Vec::<u8>::new();
            bytes.resize(item.length, 0);
            bundle.seek_read(&mut bytes, item.offset as u64)?;
            let scanned = do_scan_buffer(&bytes, item);
            match scanned {
                Err(e) => eprintln!("Failed reading {} byte file \"{}\": {}", bytes.len(), item.key, e),
                Ok(v) => found.extend(v),
                _ => ()
            }
        }
    }
    return Ok(found);
}

fn do_scan_buffer(buf: &[u8], item: ReadItem) -> Result<Vec<Rc<str>>, Box<dyn std::error::Error>>{
    let iter_res: Result<Box<dyn Iterator<Item=Rc<str>>>, Box<dyn std::error::Error>> = match item.key.extension.text {
        Some("credits") => scriptdata::scan_credits(&buf),
        Some("dialog_index") => scriptdata::scan_dialog_index(&buf),
        Some("sequence_manager") => scriptdata::scan_sequence_manager(&buf),
        Some("continent") => scriptdata::scan_continent(&buf),
        Some("continents") => scriptdata::scan_continents(&buf, Rc::from(item.key.path.text.unwrap())),
        Some("world") => scriptdata::scan_world(&buf, Rc::from(item.key.path.text.unwrap())),
        Some("mission") => scriptdata::scan_mission(&buf),
        Some("object") => xml::scan_object(&buf),
        Some("animation_state_machine") => xml::scan_animation_state_machine(&buf),
        Some("animation_subset") => xml::scan_animation_subset(&buf),
        Some("effect") => xml::scan_effect(&buf),
        _ => panic!("Selected a file {:?} to scan and then didn't scan it", item.key)
    };
    let result = iter_res.map(Iterator::collect::<Vec<_>>);
    return result;
}

