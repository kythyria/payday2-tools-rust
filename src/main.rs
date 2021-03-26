#![allow(dead_code)]

#[macro_use]
mod util;

mod diesel_hash;
mod hashindex;
mod bundles;
mod filesystem;
mod formats;
mod hashlist_scan;

use std::vec::Vec;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{Read,Write};
use std::sync::Arc;

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
        #[structopt(short="if", long="input-format")]
        input_format: Option<ConvertType>,

        ///Output format
        #[structopt(short, long, default_value="generic")]
        output_format: ConvertType,

        /// File to read
        input: String,
        /// File to write
        #[structopt(default_value="-")]
        output: String
    }
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    match opt.command {
        Command::Hash{ to_hash } => {
            for s in to_hash {
                println!("{:>016x} {:?}", diesel_hash::hash_str(&s), s)
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
        Command::Mount{ asset_dir, mountpoint } => {
            do_mount(&mountpoint, &opt.hashlist, &asset_dir)
        },
        Command::Scan{ asset_dir, output } => {
            do_scan(&opt.hashlist, &asset_dir, &output)
        },
        Command::Convert{ input, output, input_format, output_format } => {
            do_convert(&input, input_format, &output, output_format)
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
        match u64::from_str_radix(s, radix) {
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

fn do_mount(mountpoint: &str, hashlist_filename: &Option<String>, asset_dir: &str) {
    let hashlist = get_hashlist(hashlist_filename).unwrap();
    let db = get_packagedb(hashlist, asset_dir).unwrap();
    filesystem::mount_cooked_database(mountpoint, db.hashes.clone(), Arc::new(db));
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

fn do_convert(input_filename: &str, _input_type: Option<ConvertType>, output_filename: &str, output_type: ConvertType) {
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
            formats::scriptdata::lua_like::dump(&doc.unwrap(), &mut ob).unwrap();
            ob
        },
        ConvertType::Generic => {
            formats::scriptdata::generic_xml::dump(&doc.unwrap()).into_bytes()
        }
        ConvertType::Custom => {
            formats::scriptdata::custom_xml::dump(&doc.unwrap()).into_bytes()
        }
        ConvertType::Binary => {
            unimplemented!()
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