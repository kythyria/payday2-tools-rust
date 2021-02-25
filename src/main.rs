#![allow(dead_code)]

mod diesel_hash;
mod hashindex;
mod bundles;
mod util;
mod filesystem;
mod formats;
mod hashlist_scan;

use std::vec::Vec;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{Read,Write};
use std::sync::Arc;

use clap::{clap_app, arg_enum, value_t};

use hashindex::HashIndex;

arg_enum! {
    #[derive(Debug, Clone, Copy, Ord, Eq, PartialOrd, PartialEq, Hash)]
    enum ConvertType {
        Lua,
        Generic,
        Custom
    }
}

fn main() {
    let app = clap_app!(("Payday 2 CLI Tools") =>
        (version: "0.1")
        (about: "Does various things related to the game Payday 2")
        (@arg hashlist: -h --hashlist [HASHLIST] "Load hashlist from this file [default: .exe location or CWD]")
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
            (about: "Convert binary scriptdata to text")
            (@arg format: -f --format [FORMAT] possible_values(&ConvertType::variants()) default_value("generic") "Output format")
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
            let hashlist_maybe = get_hashlist(arg_matches.value_of("hashlist"));
            match hashlist_maybe {
                None => return,
                Some(hashlist) => do_unhash(hashlist, sc_args.values_of("to_unhash").unwrap().collect())
            }
        },
        ("read_packages", Some(sc_args)) => {
            let hashlist_maybe = get_hashlist(arg_matches.value_of("hashlist"));
            match hashlist_maybe {
                None => return,
                Some(hashlist) => do_readpkg(hashlist, sc_args.value_of("assetdir").unwrap())
            }
        },
        ("mount", Some(sc_args)) => {
            do_mount(sc_args.value_of("mountpoint").unwrap(), arg_matches.value_of("hashlist"), sc_args.value_of("assetdir").unwrap())
        },
        ("convert", Some(sc_args)) => {
            let in_name = sc_args.value_of("input").unwrap();
            let out_name = sc_args.value_of("output").unwrap();
            let format = value_t!(sc_args, "format", ConvertType).unwrap_or_else(|e| e.exit());
            do_convert(in_name, out_name, format);
        }
        _ => {
            println!("Unknown command, use --help for a list.");
            return;
        }
    }
    return;
}

fn get_hashlist(hashlist_filename: Option<&str>) -> Option<HashIndex> {
    match try_get_hashlist(hashlist_filename) {
        Ok(hi) => Some(hi),
        Err(e) => {
            println!("Failed to read hashlist: {}", e);
            if e.kind() == std::io::ErrorKind::NotFound {
                println!("Use --hashlist to specify the location of the hashlist");
            }
            None
        }
    }
}

fn try_get_hashlist(filename_arg: Option<&str>) -> Result<HashIndex, std::io::Error> {
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

fn do_mount(mountpoint: &str, hashlist_filename: Option<&str>, asset_dir: &str) {
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

fn do_convert(input_filename: &str, output_filename: &str, output_type: ConvertType) {
    let in_data: Vec<u8> = match input_filename {
        "-" => {
            let mut id = Vec::<u8>::new();
            std::io::stdin().read_to_end(&mut id).unwrap();
            id
        },
        name => std::fs::read(name).unwrap()
    };

    let doc = formats::scriptdata::binary::from_binary(&in_data, false);
    
    let output = match output_type {
        ConvertType::Lua => {
            let mut ob = Vec::<u8>::new();
            formats::scriptdata::lua_like::dump(&doc, &mut ob).unwrap();
            ob
        },
        ConvertType::Generic => {
            formats::scriptdata::generic_xml::dump(&doc).into_bytes()
        }
        ConvertType::Custom => {
            formats::scriptdata::custom_xml::dump(&doc).into_bytes()
        }
    };

    match output_filename {
        "-" => {
            std::io::stdout().write_all(&output).unwrap();
        },
        name => {
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(name)
                .unwrap()
                .write_all(&output)
                .unwrap()
        }
    };
}