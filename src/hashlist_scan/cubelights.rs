use rayon::prelude::*;

use crate::bundles::database::Database;
use crate::diesel_hash::{hash_str as dhash};

pub fn scan(database: &Database) -> Vec<Box<str>> {
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

    let cubelight_fmap = worlds.par_iter().flat_map(|world|{
        force_cubelight_names(world, database)
    });

    let mut found: Vec<Box<str>> = cubelight_fmap.collect();

    found.extend(worlds.iter().flat_map(|path|{
        let domeocc = format!("{}/cube_lights/dome_occlusion", path);
        let founddome = database.get_by_hashes(dhash(&domeocc), dhash(""), dhash("texture"));
        founddome.map(|_| Box::from(domeocc))
    }));

    found
}

fn force_cubelight_names<'a>(world: &'a str, db: &'a Database) -> impl rayon::iter::ParallelIterator<Item=Box<str>> +'a {
    (0..1000000).into_par_iter().filter_map(move |i| {
        let path = format!("{}/cube_lights/{}", world, i);
        let hsh = dhash(&path);
        match db.get_by_hashes(hsh, dhash(""), dhash("texture")) {
            Some(_) => Some(Box::from(path)),
            None => None
        }
    })
}