use std::fmt::Write;
use std::rc::Rc;

use rayon::prelude::*;

use crate::bundles::database::Database;
use crate::diesel_hash::hash_str as dhash;
use diesel_hash::hash::{EMPTY, MATERIAL_CONFIG, TEXTURE, UNIT};

pub fn scan_cubelights(database: &Database) -> Vec<Box<str>> {
    let worlds: Vec<_> = database.files().filter_map(|item|{
        let k = item.key();
        if k.extension.hash != dhash("world") { return None }
        if let Some(path) = k.path.text {
            if let Some(ls) = path.rfind('/') {
                return Some(&path[..ls]);
            }
        }
        None
    }).collect();

    //let candidates_list = database.unknown_path_hashes();
    //let candidates = &candidates_list;

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
            move |(buf, bl), n| {
                //buf.clear();
                buf.truncate(*bl);
                write!(buf, "{}", n).unwrap();
                let hsh = dhash(&buf);
                match database.get_by_hashes(hsh, EMPTY, TEXTURE) {
                //match candidates.contains(&hsh) {
                    Some(_) => {
                        let b = Box::<str>::from(buf.as_str());
                        Some(b)
                    },
                    None => None
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
    let known_materials: Vec<_> = database.files().filter_map(|item|{
        let k = item.key();
        if k.extension.hash != MATERIAL_CONFIG { return None }
        return k.path.text
    }).collect();

    let mut result = Vec::<Box<str>>::new();
    let alloc_len = known_materials.iter().map(|i| i.len()).max().unwrap_or_default() + 4;
    let mut buf = String::with_capacity(alloc_len);
    for mn in known_materials {
        buf.clear();
        write!(buf, "{}_thq", mn).unwrap();
        let hsh = dhash(&buf);
        match database.get_by_hashes(hsh, EMPTY, MATERIAL_CONFIG) {
            Some(_) => {
                let b = Box::<str>::from(buf.as_str());
                result.push(b);
            },
            None => ()
        }

        buf.clear();
        write!(buf, "{}_cc", mn).unwrap();
        let hsh = dhash(&buf);
        match database.get_by_hashes(hsh, EMPTY, MATERIAL_CONFIG) {
            Some(_) => {
                let b = Box::<str>::from(buf.as_str());
                result.push(b);
            },
            None => ()
        }

        buf.clear();
        write!(buf, "{}_thq_cc", mn).unwrap();
        let hsh = dhash(&buf);
        match database.get_by_hashes(hsh, EMPTY, MATERIAL_CONFIG) {
            Some(_) => {
                let b = Box::<str>::from(buf.as_str());
                result.push(b);
            },
            None => ()
        }

        buf.clear();
        write!(buf, "{}_cc_thq", mn).unwrap();
        let hsh = dhash(&buf);
        match database.get_by_hashes(hsh, EMPTY, MATERIAL_CONFIG) {
            Some(_) => {
                let b = Box::<str>::from(buf.as_str());
                result.push(b);
            },
            None => ()
        }
    }

    result
}

pub fn scan_unit_suffixes(database: &Database) -> Vec<Box<str>> {
    let known_units: Vec<_> = database.files().filter_map(|item|{
        let k = item.key();
        if k.extension.hash != UNIT { return None }
        k.path.text
    }).collect();

    let mut result = Vec::<Box<str>>::new();
    let bufsize = known_units.iter().map(|i| i.len()).max().unwrap_or_default() + "_husk".len();
    let mut buf = String::with_capacity(bufsize); 

    for un in known_units {
        buf.clear();
        write!(buf, "{}_husk", un).unwrap();
        let hsh = dhash(&buf);
        match database.get_by_hashes(hsh, EMPTY, UNIT) {
            Some(_) => {
                let b = Box::<str>::from(buf.as_str());
                result.push(b);
            },
            None => ()
        }
    }
    result
}