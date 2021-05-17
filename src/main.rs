#![allow(dead_code)]

#[macro_use]
mod util;

mod diesel_hash;
mod hashindex;
mod bundles;
mod formats;
mod hashlist_scan;

#[cfg(feature="dokan")]
mod filesystem;

use std::vec::Vec;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{Read,Write};

use anyhow::Context;
use clap::arg_enum;
use structopt::StructOpt;

use hashindex::HashIndex;

arg_enum! {
    #[derive(Debug, Clone, Copy, Ord, Eq, PartialOrd, PartialEq, Hash)]
    enum ConvertType {
        Binary,
        Lua,
        Generic,
        Custom
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name="Payday 2 CLI Tools", about="Does various things related to the game Payday 2")]
struct Opt {
    /// Path of hashlist to use. By default look in cwd and then next to the executable.
    #[structopt(short, long)]
    hashlist: Option<String>,

    #[structopt(subcommand)]
    command: Command
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Calculate Diesel hash of each argument
    Hash {
        /// String(s) to hash
        to_hash: Vec<String>
    },

    /// Look up hashes in the hashlist
    Unhash {
        /// Parse hashes as decimal numbers rather than hex
        #[structopt(short, long)]
        decimal: bool,

        /// Hashes to search for
        to_unhash: Vec<String>
    },

    /// Read package headers and don't do anything with them
    #[structopt(name="read-packages")]
    ReadPackages {
        /// Directory containing bundle_db.blb
        asset_dir: String
    },

    #[cfg(feature="dokan")]
    /// Mount packages as a volume using Dokany
    Mount {
        /// Directory containing bundle_db.blb
        asset_dir: String,
        /// Drive letter to mount on
        mountpoint: String
    },

    /// Scan packages for strings
    Scan {
        /// Directory containing bundle_db.blb
        asset_dir: String,
        /// File to write the strings to
        output: String
    },

    /// Convert between scriptdata formats
    Convert {
        /// Input format
        #[structopt(long)]
        input_format: Option<ConvertType>,

        /// Output format
        #[structopt(short, long, default_value="generic")]
        output_format: ConvertType,

        /// Print the events read by the event-based parser.
        #[structopt(short, long)]
        events: bool,

        /// File to read
        input: String,
        /// File to write
        #[structopt(default_value="-")]
        output: String
    },

    /// Parse an OIL-format model file and print all recognised information.
    Oil {
        input: String
    }
}

fn main() {
    let opt = Opt::from_args();

    match opt.command {
        Command::Hash{ to_hash } => {
            for s in to_hash {
                let h = diesel_hash::hash_str(&s);
                println!("{0:>016x} {0:>20} {1:?}", h, s)
            }
        },
        Command::Unhash{ to_unhash, decimal } => {
            if let Some(hashlist) = get_hashlist(&opt.hashlist) {
                let radix = if decimal { 10 } else { 16 };
                do_unhash(hashlist, &to_unhash, radix)
            }
        },
        Command::ReadPackages{ asset_dir } => {
            if let Some(hashlist) = get_hashlist(&opt.hashlist) {
                do_readpkg(hashlist, &asset_dir)
            }
        },
        #[cfg(feature="dokan")]
        Command::Mount{ asset_dir, mountpoint } => {
            use std::sync::Arc;

            let hashlist = get_hashlist(&opt.hashlist).unwrap();
            let db = get_packagedb(hashlist, &asset_dir).unwrap();
            filesystem::mount_cooked_database(mountpoint, db.hashes.clone(), Arc::new(db));
        },
        Command::Scan{ asset_dir, output } => {
            do_scan(&opt.hashlist, &asset_dir, &output)
        },
        Command::Convert{ input, output, input_format, output_format, events } => {
            do_convert(&input, input_format, &output, output_format, events)
        }
        Command::Oil{ input } => {
            let path: std::path::PathBuf = input.into();
            formats::oil::print_sections(&path);
        }
    };
}

fn get_hashlist(hashlist_filename: &Option<String>) -> Option<HashIndex> {
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

fn do_unhash(hashlist: hashindex::HashIndex, texts: &Vec<String>, radix: u32) {
    for s in texts {
        match diesel_hash::parse_flexibly(s, radix) {
            Ok(i) => {
                let hash_le = hashlist.get_hash(i);
                let hash_be = hashlist.get_hash(u64::from_be_bytes(i.to_le_bytes()));
                println!("{:?}", hash_le);
                if hash_be.text.is_some() {
                    println!("{:?}", hash_be);
                }
            },
            Err(()) => println!("{:?} doesn't look like a hash", s)
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

fn do_scan(hashlist_filename: &Option<String>, asset_dir: &str, outname: &str) {
    let hashlist = get_hashlist(hashlist_filename).unwrap();
    let db = get_packagedb(hashlist, asset_dir).unwrap();
    let mut outfile = std::fs::OpenOptions::new().create(true).write(true).open(outname).unwrap();
    hashlist_scan::do_scan(&db, &mut outfile).unwrap();
}

fn do_print_scriptdata(filename: &str) {
    let sd = std::fs::read(filename).unwrap();
    let doc = formats::scriptdata::binary::from_binary(&sd, false);
    let gx = formats::scriptdata::generic_xml::dump(&doc.unwrap());
    println!("{}", gx);
    //formats::scriptdata::lua_like::dump(&doc, &mut std::io::stdout()).unwrap();
    //println!("{:?}", doc.root())
}

fn do_convert(input_filename: &str, input_type: Option<ConvertType>, output_filename: &str, output_type: ConvertType, events: bool) {
    let in_data: Vec<u8> = match input_filename {
        "-" => {
            let mut id = Vec::<u8>::new();
            std::io::stdin().read_to_end(&mut id).unwrap();
            id
        },
        name => std::fs::read(name).unwrap()
    };

    if events {
        let in_text = std::str::from_utf8(&in_data).unwrap();
        let in_tree = roxmltree::Document::parse(&in_text).unwrap();
        let events = match input_type {
            Some(ConvertType::Custom) => formats::scriptdata::custom_xml::load_events(&in_tree),
            Some(ConvertType::Generic) => formats::scriptdata::generic_xml::load_events(&in_tree),
            _ => unimplemented!("Not a format supporting events")
        };
        let ok_events: Vec<_> = events.iter().filter_map(|i| i.ok()).collect();
        let err_events: Vec<_> = events.iter().filter_map(|i| i.err()).collect();
        println!("{:?}", events);
        //println!("{:?}", err_events);
    }

    let input_func = match input_type {
        Some(ConvertType::Binary) => formats::scriptdata::binary::load,
        Some(ConvertType::Custom) => formats::scriptdata::custom_xml::load,
        _ => unimplemented!("Only custom and binary are currently implemented")
    };

    let doc = input_func(&in_data).with_context(||{
        format!("Decoding \"{}\" as {:?}", input_filename, input_type)
    }).unwrap();

    

    let output_func = match output_type {
        ConvertType::Lua => formats::scriptdata::lua_like::dump,
        ConvertType::Generic => formats::scriptdata::generic_xml::dump,
        ConvertType::Custom => formats::scriptdata::custom_xml::dump,
        ConvertType::Binary => unimplemented!()
    };
    let output = output_func(&doc).into_bytes();

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