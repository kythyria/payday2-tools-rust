#![allow(dead_code)]

#[macro_use]
pub mod util;

pub mod bundles;
pub mod formats;
pub mod hashlist_scan;
pub mod filesystem;

pub use diesel_hash;
pub use diesel_hash::hashlist as hashindex;

use std::fs;
use std::path::{Path, PathBuf};

use hashindex::HashIndex;

pub fn get_packagedb<'a>(hashlist: hashindex::HashIndex, asset_dir: &Path) -> Result<bundles::database::Database, bundles::ReadError> {
    let coll = bundles::loader::load_bundle_dir(asset_dir)?;

    println!("Packages: {}", coll.1.len());
    println!("BDB Entries: {}", coll.0.files.len());
    println!();

    Ok(bundles::database::from_bdb( hashlist, &coll.0, &coll.1))
}

pub fn get_hashlist(hashlist_filename: &Option<String>) -> Option<HashIndex> {
    eprintln_time!("get_hashlist() start");
    let res = match try_get_hashlist(hashlist_filename) {
        Ok(hi) => Some(hi),
        Err(e) => {
            println!("Failed to read hashlist: {}", e);
            if e.kind() == std::io::ErrorKind::NotFound {
                println!("Use --hashlist to specify the location of the hashlist");
            }
            None
        }
    };
    eprintln_time!("get_hashlist() end");
    res
}

fn try_get_hashlist(filename_arg: &Option<String>) -> Result<HashIndex, std::io::Error> {
    if let Some(hf) = filename_arg {
        let hp = PathBuf::from(hf);
        return try_load_hashlist(&hp);
    }
    else {
        let cwd_filename = std::env::current_dir().map(|f| {
            let mut g = f.clone();
            g.push("hashlist");
            g
        });
        let exe_filename = std::env::current_exe().map(|f| {
            let mut g = f.clone();
            g.pop();
            g.push("hashlist");
            g
        });

        let hi = cwd_filename.and_then(|f| try_load_hashlist(&f));
        match hi {
            Ok(h) => Ok(h),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    exe_filename.and_then(|f| try_load_hashlist(&f))
                }
                else { Err(e) }
            }
        }
    }
}

fn try_load_hashlist(filename: &Path) -> Result<HashIndex, std::io::Error> {
    fs::read_to_string(filename).map(|c| {
        let mut hi = HashIndex::new();
        hi.load_blob(c);
        hi
    })
}