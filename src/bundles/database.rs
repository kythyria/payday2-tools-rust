use std::cmp::Ord;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::PathBuf;
use std::time::SystemTime;

use fnv::FnvHashMap;
use fnv::FnvHashSet;

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

pub struct Database<'a> {
    hashes: &'a HashIndex,
    
    // Items by their index in self.items
    item_index: FnvHashMap<(u64, u64, u64), u32>,
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
    //packages: Vec<u32>,
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

impl<'a> Database<'a> {
    pub fn get_by_hashes(&self, path: u64, language: Option<u64>, extension: Option<u64>) -> Option<DatabaseItem> {
        let query = (path, language.unwrap_or(diesel_hash::EMPTY), extension.unwrap_or(diesel_hash::EMPTY));
        let idx = self.item_index.get(&query)?;
        return Some(self.get_by_inode(*idx));
    }

    fn get_by_inode(&'a self, inode_number: u32) -> DatabaseItem {
        DatabaseItem {
            db: self,
            item_number: inode_number
        }
    }

    pub fn print_stats(&self) {
        let mut foldercount = 0;
        for i in &self.items {
            match i.specifics {
                ItemRecordSpecifics::Folder(_) => foldercount += 1,
                _ => {}
            }
        }

        println!("Items: {}", self.items.len());
        println!("Folders: {}", foldercount);
        println!("Packages: {}", self.packages.len());
    }
}

pub struct DatabaseItem<'a> {
    db: &'a Database<'a>,
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
            /*ItemRecordSpecifics::Folder(folder) => {
                let packid = folder.packages.get(0).unwrap();
                let package = self.db.packages.get(*packid as usize).unwrap();
                package.last_modified
            }*/
            _ => SystemTime::UNIX_EPOCH
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
    db: &'a Database<'a>,
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

pub fn from_bdb<'a>(hashlist: &'a mut HashIndex, bdb: &bundledb_reader::BundleDbFile, packages: &Vec<loader::ParsedBundle>) -> Database<'a> {
    let mut items = Vec::<ItemRecord>::new();
    let mut itemkeys_by_file_id = FnvHashMap::<u32, (u64, u64, u64)>::default();
    let mut folder_paths = FnvHashSet::<String>::default();
    items.reserve(bdb.files.len());
    itemkeys_by_file_id.reserve(bdb.files.len());

    for bdbe in &bdb.files {
        let le = match bdbe.lang_id {
            0 => diesel_hash::EMPTY,
            id => bdb.languages.iter().find(|i| i.id == id).unwrap().hash
        };
        items.push(ItemRecord {
            path: bdbe.path,
            language: le,
            extension: bdbe.extension,
            specifics: ItemRecordSpecifics::File(FileRecord {
                packages: Vec::new()
            })
        });
        itemkeys_by_file_id.insert(bdbe.file_id, (bdbe.path, le, bdbe.extension));

        let hs = hashlist.get_hash(bdbe.path);
        match hs.text {
            None => continue,
            Some(path) => {
                for (i, c) in path.char_indices() {
                    if c == '/' {
                        let path_owned = path[0..i].to_owned();
                        folder_paths.insert(path_owned);
                    }
                }
            }
        }
    }

    // root folder
    if !folder_paths.contains("") {
        items.push(ItemRecord {
            path: diesel_hash::EMPTY,
            language: diesel_hash::EMPTY,
            extension: diesel_hash::EMPTY,
            specifics: ItemRecordSpecifics::Folder(FolderRecord {
                //packages: Vec::new(),
                first_child: 0,
                child_count: 0
            })
        });
    }

    for p in folder_paths.drain() {
        let h = hashlist.intern(p);
        items.push(ItemRecord {
            path: h.hash,
            language: diesel_hash::EMPTY,
            extension: diesel_hash::EMPTY,
            specifics: ItemRecordSpecifics::Folder(FolderRecord {
                //packages: Vec::new(),
                first_child: 0,
                child_count: 0
            })
        });
    }

    items.sort_by_cached_key(|item| {
        match hashlist.get_hash(item.path).text {
            None => PathSortKey::Unhashed(item.path),
            Some(path) => {
                if path == "" {
                    PathSortKey::Root
                }
                else {
                    let components = path.split('/').collect();
                    PathSortKey::Hashed(components)
                }
            }
        }
    });

    let mut item_index = FnvHashMap::<(u64, u64, u64), u32>::default();

    // the list is now in breadth-first order
    // now we have to tell each folder where its children are.
    // 
    // Breadth first order means that we see folders in the same order that we would
    // if we scanned each item and calculated its parent

    let mut current_folder : usize = 0;
    let mut current_item : usize = 1;
    let mut current_folder_path = "";
    let mut current_folder_start = 1;
    let mut current_folder_len = 0;

    while current_item < items.len() {
        
        let ci = items.get(current_item).unwrap();
        let ci_path_hs = hashlist.get_hash(ci.path);

        item_index.insert((ci.path, ci.language, ci.extension), current_item.try_into().unwrap());

        // it doesn't matter what the default is, but it has to be something.
        // with no slashes, since unhashed things end up in the root
        let ci_path = ci_path_hs.text.unwrap_or("0000000000000000");

        // if there's no slashes we're in the root, so we need to match ""
        let lastslash = ci_path.rfind('/').unwrap_or(0);

        if current_folder_path == &ci_path[0..lastslash] {
            current_folder_len += 1;
        }
        else {
            let cfs = &mut items.get_mut(current_folder).unwrap().specifics;
            match cfs {
                ItemRecordSpecifics::File(_) => panic!("Current folder is a file"),
                ItemRecordSpecifics::Folder(f) => {
                    f.first_child = current_folder_start;
                    f.child_count = current_folder_len;
                }
            }

            current_folder_start = current_item.try_into().unwrap();
            current_folder_len = 1; // folders are only implied in the bdb, by paths having
                                    // slashes. So any folder definitely has one entry, and
                                    // not doing this actually makes an off by one error as
                                    // the entry wouldn't be counted.

            loop {
                current_folder += 1;
                let next_folder = items.get(current_folder).unwrap();
                if let ItemRecordSpecifics::Folder(_) = next_folder.specifics {
                    current_folder_path = hashlist.get_hash(next_folder.path).text.unwrap();
                    break;
                }
            }
        }

        current_item += 1;
    }

    // Now we need to line up the package entries with the items. There's probably a
    // much better way to do this.
    let mut package_catalog = Vec::<PackageRecord>::new();
    package_catalog.reserve_exact(packages.len());
    let mut package_index = FnvHashMap::<u64, u32>::default();
    package_index.reserve(packages.len());

    for pack in packages {
        let mut pr = PackageRecord {
            data_path: pack.data_path.to_owned(),
            last_modified: pack.last_modified,
            package_id: pack.package_id,
            files: Vec::new()
        };

        pr.files.reserve_exact(pack.header.entries.len());
        
        for entry in &pack.header.entries {
            let fk = itemkeys_by_file_id.get(&entry.file_id).unwrap();
            let fid = item_index.get(&fk).unwrap();
            
            pr.files.push( PackageEntryRecord {
                item_number: *fid,
                offset: entry.offset,
                length: entry.length
            });
            
            let item = items.get_mut(*fid as usize).unwrap();
            match &mut item.specifics {
                ItemRecordSpecifics::Folder(_) => panic!(),
                ItemRecordSpecifics::File(fs) => fs.packages.push(FileToPackage {
                    package_number: package_catalog.len().try_into().unwrap(),
                    file_number: (pr.files.len() - 1).try_into().unwrap()
                })
            };
        }
        package_index.insert(pack.package_id, package_catalog.len().try_into().unwrap());
        package_catalog.push(pr);
    }

    Database {
        hashes: hashlist,
        item_index,
        items,
        packages: package_catalog,
        package_index
    }
}

#[derive(PartialEq, Eq)]
enum PathSortKey<'a> {
    Unhashed(u64),
    Hashed(Vec<&'a str>),
    Root
}

impl<'a> PartialOrd<PathSortKey<'a>> for PathSortKey<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl<'a> Ord for PathSortKey<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            PathSortKey::Root => match other {
                PathSortKey::Root => Ordering::Equal,
                _ => Ordering::Less
            },
            PathSortKey::Unhashed(sh) => match other {
                PathSortKey::Root => Ordering::Greater,
                PathSortKey::Unhashed(oh) => sh.cmp(oh),
                PathSortKey::Hashed(_) => Ordering::Less
            }
            PathSortKey::Hashed(sc) => match other {
                PathSortKey::Root => Ordering::Greater,
                PathSortKey::Unhashed(_) => Ordering::Greater,
                PathSortKey::Hashed(oc) => {
                    match sc.len().cmp(&oc.len()) {
                        Ordering::Equal => sc.cmp(oc),
                        c => c
                    }
                }
            }
        }
    }
}

pub fn print_record_sizes() {
    println!("bundles::database");
    println!("    ItemRecord: {}", std::mem::size_of::<ItemRecord>());
    println!("    SortKey: {}", std::mem::size_of::<PathSortKey>());
    println!();
    println!("Vec<&str>: {}", std::mem::size_of::<Vec<&str>>());
}