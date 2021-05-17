use std::sync::Arc;

use structopt::StructOpt;

use pd2tools_rust::filesystem;

#[derive(Debug, StructOpt)]
#[structopt(name="Payday 2 BundleFS", about="Mount asset bundles from Payday 2 as a Dokany filesystem")]
struct Opt {
    /// Path of hashlist to use. By default look in cwd and then next to the executable.
    #[structopt(short, long)]
    hashlist: Option<String>,

    /// Directory containing bundle_db.blb
    asset_dir: String,
    /// Drive letter to mount on
    mountpoint: String
}

fn main() {
    let opt = Opt::from_args();

    let hashlist = pd2tools_rust::get_hashlist(&opt.hashlist).unwrap();
    let db = pd2tools_rust::get_packagedb(hashlist, &opt.asset_dir).unwrap();
    filesystem::mount_cooked_database(&opt.mountpoint, db.hashes.clone(), Arc::new(db));
}