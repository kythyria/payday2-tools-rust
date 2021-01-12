mod diesel_hash;
mod hashindex;
mod testfs;
mod bundles;
mod read_util;

use std::env;
use std::vec::Vec;
use std::fs;
use hashindex::HashIndex;

fn main() {
    let argv : Vec<String> = env::args().collect();
    
    let filename = &argv[1];
    let contents = fs::read_to_string(filename).unwrap();

    let hashlist = hashindex::BlobHashIndex::new(contents);

    for item in (&argv).iter().skip(2) {
        println!("{}", &hashlist.intern(item))
    }

    let bytes = fs::read(&argv[2]).unwrap();
    let pidx = bundles::bundledb_reader::read_bundle_db(&bytes);
    println!("files: {}, languages: {}", pidx.files.len(), pidx.languages.len());

}
