#![allow(dead_code)]

mod diesel_hash;
mod hashindex;
mod bundles;
mod read_util;
mod filesystem;

use std::vec::Vec;
use std::fs;
use std::sync::Arc;

use clap::{App, Arg, SubCommand};

use hashindex::HashIndex;

fn main() {
    let app = App::new("Payday 2 CLI Tools")
        .version("0.1")
        //.author("Grey Heron")
        .about("Does various things related to the game Payday 2")
        .arg(Arg::with_name("hashlist")
            .short("h")
            .long("hashlist")
            .value_name("FILE")
            .help("Load hashlist from this file")
            .takes_value(true)
            .default_value("./hashlist"))
        .subcommand(SubCommand::with_name("hash")
            .about("Calculate diesel hash of arguments")
            .arg(Arg::with_name("to_hash")
                .takes_value(true)
                .value_name("STRING")
                .multiple(true)))
        .subcommand(SubCommand::with_name("unhash")
            .about("Given diesel hashes, look them up in the hashlist")
            .arg(Arg::with_name("to_unhash")
                .takes_value(true)
                .value_name("HASH")
                .multiple(true)))
        .subcommand(SubCommand::with_name("read-packages")
            .arg(Arg::with_name("assetdir")
                .takes_value(true)
                .value_name("ASSET_DIR")
                .required(true)))
        .subcommand(SubCommand::with_name("struct-sizes"))
        .subcommand(SubCommand::with_name("mount")
            .arg(Arg::with_name("assetdir")
                .takes_value(true)
                .value_name("ASSET_DIR")
                .required(true)
                .help("Path of directory with bundle files"))
            .arg(Arg::with_name("mountpoint")
                .takes_value(true)
                .value_name("MOUNT_POINT")
                .required(true)
                .help("Drive letter to mount on")));
    
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
        ("struct-sizes", Some(_)) => {
            bundles::database::print_record_sizes();
        }
        ("mount", Some(sc_args)) => {
            do_mount(sc_args.value_of("mountpoint").unwrap(), arg_matches.value_of("hashlist").unwrap(), sc_args.value_of("assetdir").unwrap())
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

fn get_packagedb<'a>(hashlist: &'a mut hashindex::HashIndex, asset_dir: &str) -> Result<bundles::database::Database<'a>, bundles::ReadError> {
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

fn do_readpkg(mut hashlist: hashindex::HashIndex, asset_dir: &str) {
    let r_bdb = get_packagedb(&mut hashlist, asset_dir);

    match r_bdb {
        Err(e) => println!("Couldn't read asset database: {:?}", e),
        Ok(db) => {
            db.print_stats();
        }
    }
}

fn do_mount(mountpoint: &str, hashlist_filename: &str, asset_dir: &str) {
    let mut hashlist = get_hashlist(hashlist_filename).unwrap();
    let db = get_packagedb(&mut hashlist, asset_dir).unwrap();
    filesystem::mount_raw_database(mountpoint, Arc::new(db));
}