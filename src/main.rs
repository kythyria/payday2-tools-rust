mod diesel_hash;
mod hashindex;

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
}
