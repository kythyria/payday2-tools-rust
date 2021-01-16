mod diesel_hash;
mod hashindex;
mod bundles;
mod read_util;

use std::env;
use std::vec::Vec;
use std::fs;

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
                .multiple(true)));
    
    let arg_matches = app.get_matches();
    
    let hashlist_filename = arg_matches.value_of("hashlist").unwrap();
    let contents = match fs::read_to_string(hashlist_filename) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to read hashlist: {}", e);
            if e.kind() == std::io::ErrorKind::NotFound {
                println!("Use --hashlist to specify the location of the hashlist");
            }
            return;
        }
    };
    let mut hashlist = hashindex::BlobHashIndex::new(contents);

    match arg_matches.subcommand() {
        ("hash", Some(sc_args)) => {
            do_hash(sc_args.values_of("to_hash").unwrap().collect())
        },
        ("unhash", Some(sc_args)) => {
            do_unhash(&hashlist, sc_args.values_of("to_unhash").unwrap().collect());
        },
        _ => {
            println!("Unknown command, use --help for a list.");
            return;
        }
    }
    return;
}

fn do_hash(texts: Vec<&str>) {
    for s in texts {
        println!("{:>016x} {:?}", diesel_hash::hash_str(s), s);
    }
}

fn do_unhash(hashlist: &dyn hashindex::HashIndex, texts: Vec<&str>) {
    for s in texts {
        match u64::from_str_radix(s, 16) {
            Err(e) => println!("{:?} doesn't look like a hash ({})", s, e),
            Ok(i) => println!("{:?}", hashlist.get_hash(i))
        }
    }
}