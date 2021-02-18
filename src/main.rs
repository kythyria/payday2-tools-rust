#![allow(dead_code)]

mod diesel_hash;
mod hashindex;
mod bundles;
mod util;
mod filesystem;
mod formats;

use std::vec::Vec;
use std::fs;
use std::sync::Arc;

use clap::{clap_app};

use hashindex::HashIndex;

fn main() {
    
    let app = clap_app!(("Payday 2 CLI Tools") =>
        (version: "0.1")
        (about: "Does various things related to the game Payday 2")
        (@arg hashlist: -h --hashlist [HASHLIST] default_value("./hashlist") "Load hashlist from this file")
        (@subcommand hash =>
            (about: "Calculate diesel hash of arguments")
            (@arg to_hash: <STRING>... "String to hash")
        )
        (@subcommand unhash =>
            (about: "Given diesel hashes, look them up in the hashlist")
            (@arg to_unhash: <HASH>... "Hash to look up")
        )
        (@subcommand read_packages =>
            (about: "Reads package headers and doesn't do anything with them")
            (@arg assetdir: <ASSET_DIR> "Directory containing bundle_db.blb")
        )
        (@subcommand mount =>
            (about: "Mount packages as a volume using Dokany")
            (@arg assetdir: <ASSET_DIR> "Directory containing bundle_db.blb")
            (@arg mountpoint: <MOUNT_POINT> "Drive letter to mount on")
        )
        (@subcommand convert => 
            (about: "Converts between scriptdata formats")
            (@arg format: -f --format [FORMAT] possible_value[lua generic_xml] "Output format")
            (@arg input: <INPUT> "File to read or - for stdin")
            (@arg output: [OUTPUT] default_value("-") "File to write, - for stdout")
        )
    );
    let arg_matches = app.get_matches();

    match arg_matches.subcommand() {
        ("hash", Some(sc_args)) => {
            do_hash(sc_args.values_of("to_hash").unwrap().collect())
        },
        ("unhash", Some(sc_args)) => {
            let hashlist_maybe = get_hashlist(arg_matches.value_of("hashlist").unwrap());
            match hashlist_maybe {
                None => return,
                Some(hashlist) => do_unhash(hashlist, sc_args.values_of("to_unhash").unwrap().collect())
            }
        },
        ("read-packages", Some(sc_args)) => {
            let hashlist_maybe = get_hashlist(arg_matches.value_of("hashlist").unwrap());
            match hashlist_maybe {
                None => return,
                Some(hashlist) => do_readpkg(hashlist, sc_args.value_of("assetdir").unwrap())
            }
        },
        ("mount", Some(sc_args)) => {
            do_mount(sc_args.value_of("mountpoint").unwrap(), arg_matches.value_of("hashlist").unwrap(), sc_args.value_of("assetdir").unwrap())
        },
        ("print-scriptdata", Some(sc_args)) => {
            do_print_scriptdata(sc_args.value_of("input").unwrap())
        }
        _ => {
            println!("Unknown command, use --help for a list.");
            return;
        }
    }
    return;
}

fn get_hashlist(hashlist_filename: &str) -> Option<HashIndex> {
    match fs::read_to_string(hashlist_filename) {
        Ok(c) => {
            let mut hi = HashIndex::new();
            hi.load_blob(c);
            Some(hi)
        }
        Err(e) => {
            println!("Failed to read hashlist: {}", e);
            if e.kind() == std::io::ErrorKind::NotFound {
                println!("Use --hashlist to specify the location of the hashlist");
            }
            None
        }
    }
}

fn get_packagedb<'a>(hashlist: hashindex::HashIndex, asset_dir: &str) -> Result<bundles::database::Database, bundles::ReadError> {
    let path = std::path::PathBuf::from(asset_dir);
    let coll = bundles::loader::load_bundle_dir(&path)?;

    println!("Packages: {}", coll.1.len());
    println!("BDB Entries: {}", coll.0.files.len());
    println!();

    Ok(bundles::database::from_bdb( hashlist, &coll.0, &coll.1))
}

fn do_hash(texts: Vec<&str>) {
    for s in texts {
        println!("{:>016x} {:?}", diesel_hash::hash_str(s), s);
    }
}

fn do_unhash(hashlist: hashindex::HashIndex, texts: Vec<&str>) {
    for s in texts {
        match u64::from_str_radix(s, 16) {
            Err(e) => println!("{:?} doesn't look like a hash ({})", s, e),
            Ok(i) => println!("{:?}", hashlist.get_hash(i))
        }
    }
}

fn do_readpkg(hashlist: hashindex::HashIndex, asset_dir: &str) {
    let r_bdb = get_packagedb(hashlist, asset_dir);

    match r_bdb {
        Err(e) => println!("Couldn't read asset database: {:?}", e),
        Ok(db) => {
            db.print_stats();
        }
    }
}

fn do_mount(mountpoint: &str, hashlist_filename: &str, asset_dir: &str) {
    let hashlist = get_hashlist(hashlist_filename).unwrap();
    let db = get_packagedb(hashlist, asset_dir).unwrap();
    filesystem::mount_cooked_database(mountpoint, db.hashes.clone(), Arc::new(db));
}

fn do_print_scriptdata(filename: &str) {
    let sd = std::fs::read(filename).unwrap();
    let doc = formats::scriptdata::binary::from_binary(&sd, false);
    let gx = formats::scriptdata::generic_xml::dump(&doc);
    println!("{}", gx);
    //formats::scriptdata::lua_like::dump(&doc, &mut std::io::stdout()).unwrap();
    //println!("{:?}", doc.root())
}