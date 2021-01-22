use std::cell::Ref;
use std::collections::HashMap;
use std::collections::HashSet;
use std::cmp::Ord;
use std::path::PathBuf;
use std::time::SystemTime;

use fnv::FnvHashMap;

use crate::hashindex::HashIndex;
use crate::hashindex::HashedStr;
use crate::diesel_hash;

use super::bundledb_reader;
use super::loader;

/* The layout of this struct bears some explanation.
Because we're obsessing with memory usage, we want to be compact.
We also know the main operations are:
 - get a *specific* item, but we don't know whether we want a file or folder
   (eg, because stat(2).)
 
 - enumerate the children of an item, which we assume is a folder, if only
   because the client checked first.

 - get which packages contain an item
 - get which items are in a package

So what we do is store the items, including folders, in a Vec, sorted into
some order where all the direct children of an item are together. Breadth
first traversal of the overall folder tree, for instance. Then we have an
index of path/lang/ext to where in that array the item is.

*/

pub struct Database {
    hashes: HashIndex,
    
    // Items by their index in self.items
    item_index: HashMap<(u64, u64, u64), u32>,
    items: Vec<ItemRecord>,
    
    // Packages by their index in self.pacakges
    package_index: FnvHashMap<u64, u32>,
    packages: Vec<PackageRecord>
}

struct ItemRecord {
    path: u64,
    language: u64,
    extension: u64,
    specifics: ItemRecordSpecifics
}

enum ItemRecordSpecifics {
    File(FileRecord),
    Folder(FolderRecord)
}

struct FileRecord {
    packages: Vec<FileToPackage>
}

struct FileToPackage {
    package_number: u32,
    file_number: u32
}

struct FolderRecord {
    packages: Vec<u32>,
    first_child: u32,
    child_count: u32
}

struct PackageRecord {
    package_id: u64,
    data_path: PathBuf,
    last_modified: SystemTime,
    files: Vec<PackageEntryRecord>
}

struct PackageEntryRecord {
    item_number: u32,
    offset: u32,
    length: u32
}

impl<'a> Database {
    pub fn get_by_hashes(&self, path: u64, language: Option<u64>, extension: Option<u64>) -> Option<DatabaseItem> {
        let query = (path, language.unwrap_or(diesel_hash::EMPTY), extension.unwrap_or(diesel_hash::EMPTY));
        let idx = self.item_index.get(&query)?;
        return Some(self.get_by_inode(*idx));
    }

    fn get_by_inode(&self, inode_number: u32) -> DatabaseItem {
        DatabaseItem {
            db: self,
            item_number: inode_number
        }
    }
}

pub struct DatabaseItem<'a> {
    db: &'a Database,
    item_number: u32
}

impl DatabaseItem<'_> {
    fn item(&self) -> &ItemRecord {
        self.db.items.get(self.item_number as usize).unwrap()
    }

    pub fn path(&self) -> HashedStr {
        let item = self.item();
        let hash = self.db.hashes.get_hash(item.path);
        return hash;
    }

    pub fn extension(&self) -> Option<HashedStr> {
        let item = self.item();
        let hash = self.db.hashes.get_hash(item.extension);
        return match hash.hash {
            diesel_hash::EMPTY => None,
            _ => Some(hash)
        };
    }

    pub fn language(&self) -> Option<HashedStr> {
        let item = self.item();
        let hash = self.db.hashes.get_hash(item.language);
        return match hash.hash {
            diesel_hash::EMPTY => None,
            _ => Some(hash)
        };
    }

    pub fn last_modified(&self) -> SystemTime {
        let item = self.item();
        match &item.specifics {
            ItemRecordSpecifics::File(file) => {
                let packref = file.packages.get(0).unwrap();
                let package = self.db.packages.get(packref.package_number as usize).unwrap();
                package.last_modified
            },
            ItemRecordSpecifics::Folder(folder) => {
                let packid = folder.packages.get(0).unwrap();
                let package = self.db.packages.get(*packid as usize).unwrap();
                package.last_modified
            }
        }
    }

    pub fn item_type(&self) -> ItemType {
        let item = self.item();
        match item.specifics {
            ItemRecordSpecifics::File(_) => ItemType::File,
            ItemRecordSpecifics::Folder(_) => ItemType::Folder
        }
    }

    pub fn children<'a>(&'a self) -> ChildIterator<'a> {
        let item = self.item();
        match &item.specifics {
            ItemRecordSpecifics::File(_) => ChildIterator {
                db: self.db,
                current_index: 0,
                end_index: 0
            },
            ItemRecordSpecifics::Folder(folder) => ChildIterator {
                db: self.db,
                current_index: folder.first_child,
                end_index: folder.first_child + folder.child_count
            }
        }
     }

    pub fn contents(&self) -> Option<&dyn std::io::Read> { None }
}

pub enum ItemType {
    File,
    Folder
}

pub struct ChildIterator<'a> {
    db: &'a Database,
    current_index: u32,
    end_index: u32,
}

impl<'a> Iterator for ChildIterator<'a> {
    type Item = DatabaseItem<'a>;
    fn next(&mut self) -> Option<DatabaseItem<'a>> {
        if self.current_index >= self.end_index {
            None
        }
        else {
            let thing = self.db.get_by_inode(self.current_index);
            self.current_index += 1;
            Some(thing)
        }
    }
}

pub fn from_bdb(hashlist: HashIndex, bdb: &bundledb_reader::BundleDbFile, packages: &Vec<loader::ParsedBundle>) -> Database {
    let mut items = Vec::<ItemRecord>::new();
    items.reserve(bdb.files.len());

    for bdbe in &bdb.files {
        let le = bdb.languages.get(bdbe.lang_id as usize).unwrap();
        items.push(ItemRecord {
            path: bdbe.path,
            language: le.hash,
            extension: bdbe.extension,
            specifics: ItemRecordSpecifics::File(FileRecord {
                packages: Vec::new()
            })
        });
        match hashlist.get_hash(bdbe.path).text {
            None => {},
            Some(path) => add_folder_names(&hashlist, path)
        };
    }

    unimplemented!();
}

fn add_folder_names<'a>(hashlist: &HashIndex, path: Ref<'a,str>) {
    for (i,c) in path.char_indices() {
        if c == '/' {
            hashlist.intern(&path[0..i]);
        }
    }
}

#[derive(PartialOrd, PartialEq, Ord, Eq)]
enum PathSortKey<'a> {
    Unhashed(u64),
    Hashed(Vec<&'a str>)
}

pub fn print_record_sizes() {
    println!("bundles::database");
    println!("    ItemRecord: {}", std::mem::size_of::<ItemRecord>());
    println!("    SortKey: {}", std::mem::size_of::<PathSortKey>());
    println!();
    println!("Vec<&str>: {}", std::mem::size_of::<Vec<&str>>());
}