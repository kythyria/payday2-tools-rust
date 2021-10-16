use std::fmt::Write;
use std::rc::Rc;

use fnv::FnvHashSet;
use rayon::prelude::*;

use crate::bundles::database::Database;
use crate::diesel_hash::hash_str as dhash;
use diesel_hash::hash::{EMPTY, MATERIAL_CONFIG, TEXTURE, UNIT};

pub fn scan_cubelights(database: &Database) -> Vec<Box<str>> {
    let mut hashes_to_find = FnvHashSet::default();
    let worlds: Vec<_> = database.files().filter_map(|item|{
        let k = item.key();
        hashes_to_find.insert(k.path.hash);
        if k.extension.hash != dhash("world") { return None }
        if let Some(path) = k.path.text {
            if let Some(ls) = path.rfind('/') {
                return Some(&path[..ls]);
            }
        }
        None
    }).collect();

    let cubelight_fmap = worlds.par_iter().flat_map(|world| {
        let alloc_len = world.len() + "/cube_lights/".len() + 7;
        let world = *world;
        (0..1000000).into_par_iter().map_init(
            move ||{ 
                let mut st = String::with_capacity(alloc_len);
                write!(st, "{}/cube_lights/", world).unwrap();
                let bl = st.len();
                (st, bl)
            },
            |(buf, bl), n| {
                //buf.clear();
                buf.truncate(*bl);
                write!(buf, "{}", n).unwrap();
                let hsh = dhash(&buf);
                //match database.get_by_hashes(hsh, EMPTY, TEXTURE) {
                match hashes_to_find.contains(&hsh) {
                    true => { //Some(_) => {
                        let b = Box::<str>::from(buf.as_str());
                        Some(b)
                    },
                    false => None//None => None
                }
            }
        ).filter_map(|i| i)
    });

    let mut found: Vec<Box<str>> = cubelight_fmap.collect();

    found.extend(worlds.iter().flat_map(|path|{
        let domeocc = format!("{}/cube_lights/dome_occlusion", path);
        let founddome = database.get_by_hashes(dhash(&domeocc), EMPTY, TEXTURE);
        founddome.map(|_| Box::from(domeocc))
    }));

    found
}

pub fn scan_mat_suffixes(database: &Database) -> Vec<Box<str>> {
    scan_suffixes_for_type(database, &[MATERIAL_CONFIG], &[
        "_thq", "_cc", "_thq_cc", "_cc_thq", "_contour"
    ])
}

pub fn scan_unit_suffixes(database: &Database) -> Vec<Box<str>> {
    scan_suffixes_for_type(database, &[UNIT], &["_husk"])
}

pub fn scan_texture_suffixes(database: &Database) -> Vec<Box<str>> {
    let mut hashes_to_find = FnvHashSet::<u64>::default();
    let mut known_paths = FnvHashSet::<&str>::default();
    let mut known_suffixes = FnvHashSet::<&str>::default();
    for file in database.files() {
        let k = file.key();
        hashes_to_find.insert(k.path.hash);
        if [MATERIAL_CONFIG, TEXTURE].contains(&k.extension.hash) {
            if let Some(p) = k.path.text {
                known_paths.insert(p);
                if let Some((stem, suffix)) = p.rsplit_once('_') {
                    known_paths.insert(stem);
                    known_suffixes.insert(suffix);
                }
            }
        }
    }
    
    eprintln!("Candidates {} {}", known_paths.len(), known_suffixes.len());

    let path_len = known_paths.iter().map(|i| i.len()).max().unwrap_or_default();
    let suffix_len = known_suffixes.iter().map(|i| i.len()).max().unwrap_or_default();
    let buf_size = path_len + 1 + suffix_len;

    let looper = known_paths.into_par_iter().map_init(
        || String::with_capacity(buf_size), 
        |buf, path| {
            let mut inner_result = FnvHashSet::<Box<str>>::default();
            
            buf.clear();
            buf.push_str(path);

            if hashes_to_find.contains(&dhash(buf.as_str())) {
                inner_result.insert(Box::<str>::from(buf.as_str()));
            }

            //insert_if_exists(&mut inner_result, database, &[MATERIAL_CONFIG, TEXTURE], buf.as_str());

            buf.push('_');
            for suffix in &known_suffixes {
                buf.truncate(path.len()+1);
                buf.push_str(suffix);

                if hashes_to_find.contains(&dhash(buf.as_str())) {
                    inner_result.insert(Box::<str>::from(buf.as_str()));
                }
                
                //insert_if_exists(&mut inner_result, database, &[MATERIAL_CONFIG, TEXTURE], buf.as_str());
            }

            inner_result
        }
    ).reduce(FnvHashSet::<Box<str>>::default, |mut a,b| {
        a.extend(b.into_iter());
        a
    });

    eprintln!("{}", looper.len());
    looper.into_iter().collect()
}

fn scan_suffixes_for_type(database: &Database, filetypes: &[u64], suffixes: &[&str]) -> Vec<Box<str>> {
    let known_paths: Vec<_> = database.files().filter_map(|item|{
        let k = item.key();
        if filetypes.contains(&k.extension.hash) {
            k.path.text
        } 
        else {
            None
        }
    }).collect();

    let bufsize = known_paths.iter().map(|i| i.len()).max().unwrap_or_default()
        + suffixes.iter().map(|i| i.len()).max().unwrap_or_default();
    let mut result = Vec::<Box<str>>::new();
    let mut buf = String::with_capacity(bufsize); 

    for item in known_paths {
        buf.clear();
        buf.push_str(item);
        for suffix in suffixes {
            buf.truncate(item.len());
            buf.push_str(suffix);
            insert_if_exists(&mut result, database, filetypes, buf.as_str());
        }
    }
    result
}

fn insert_if_exists<D: Extend<Box<str>>>(dest: &mut D, database: &Database, filetypes: &[u64], path: &str) {
    let hsh = dhash(path);
    for filetype in filetypes {
        if let Some(_) = database.get_by_hashes(hsh, EMPTY, *filetype) {
            let b = Box::<str>::from(path);
            dest.extend(std::iter::once(b));
        }
    }
}