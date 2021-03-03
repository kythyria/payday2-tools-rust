use std::cmp::Ord;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::path::PathBuf;
use std::path::Path;
use std::sync::Arc;
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct HashKey {
    pub path: u64,
    pub language: u64,
    pub extension: u64
}
impl From<HashStrKey<'_>> for HashKey {
    fn from(src: HashStrKey) -> HashKey { HashKey { path: src.path.hash, language: src.language.hash, extension: src.extension.hash } }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct HashStrKey<'a> {
    pub path: HashedStr<'a>,
    pub language: HashedStr<'a>,
    pub extension: HashedStr<'a>
}
impl<'a> HashStrKey<'a> {
    pub fn from_hashes(hashlist: &'a HashIndex, key: (u64, u64, u64)) -> HashStrKey<'a> {
        HashStrKey {
            path: hashlist.get_hash(key.0),
            language: hashlist.get_hash(key.1),
            extension: hashlist.get_hash(key.2)
        }
    }
}

pub struct Database {
    pub hashes: Arc<HashIndex>,
    
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

impl<'a> Database {
    pub fn get_by_str(&self, path: &str, language: &str, extension: &str) -> Option<DatabaseItem> {
        self.get_by_hashes(diesel_hash::hash_str(path), diesel_hash::hash_str(language), diesel_hash::hash_str(extension))
    }

    pub fn get_by_hashes(&self, path: u64, language: u64, extension: u64) -> Option<DatabaseItem> {
        let query = (path, language, extension);
        let idx = self.item_index.get(&query)?;
        return Some(self.get_by_inode(*idx));
    }

    pub fn get_by_inode(&'a self, inode_number: u32) -> DatabaseItem {
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
        println!("{}", self.item_index.contains_key(&(diesel_hash::EMPTY,diesel_hash::EMPTY,diesel_hash::EMPTY)));
    }

    pub fn filter_key_sort_physical(&self, cond: fn(HashStrKey) -> bool) -> Vec<(&Path, Vec<ReadItem>)> {
        // 0: path, 1: total bytes to read from this bundle, 2: files to read.
        let mut packs = Vec::<(&Path, usize, Vec<ReadItem>)>::with_capacity(self.packages.len());

        for pkg in self.packages.iter() {
            let items: Vec<ReadItem> = pkg.files.iter().filter_map(|per| {
                let item = &self.items[per.item_number as usize];
                let key = HashStrKey::from_hashes(&self.hashes, (item.path, item.language, item.extension));
                if !cond(key) { return None }

                match &item.specifics {
                    ItemRecordSpecifics::Folder(_) => None,
                    ItemRecordSpecifics::File(_) => Some(ReadItem {
                        key,
                        last_modified: pkg.last_modified,
                        offset: per.offset as usize,
                        length: per.length as usize
                    })
                }
            }).collect();

            if items.len() == 0 { continue; }

            let byte_count = items.iter().fold(0, |m,v| m + v.length);
            packs.push((&pkg.data_path, byte_count, items));
        }

        packs.sort_unstable_by(|x, y| std::cmp::Ord::cmp(&y.1, &x.1));

        let mut seen_keys = FnvHashSet::<HashKey>::default();

        let filtered_packs: Vec<(&Path, Vec<ReadItem>)> = packs.iter().filter_map(|(path, _, items)| {
            let mut filtered_items = Vec::<ReadItem>::new();
            for item in items.iter() {
                if seen_keys.insert(HashKey::from(item.key)) {
                    filtered_items.push(item.clone());
                }
            };
            filtered_items.sort_by(|x,y| std::cmp::Ord::cmp(&x.offset, &y.offset) );
            if filtered_items.len() > 0 {
                Some((*path, filtered_items))
            }
            else {
                None
            }
        }).collect();

        return filtered_packs;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ReadItem<'a> {
    pub key: HashStrKey<'a>,
    pub last_modified: SystemTime,
    pub offset: usize,
    pub length: usize
}

pub struct DatabaseItem<'a> {
    db: &'a Database,
    item_number: u32
}

impl<'a> DatabaseItem<'a> {
    fn item(&self) -> &ItemRecord {
        self.db.items.get(self.item_number as usize).unwrap()
    }

    pub fn key(&self) -> (HashedStr, HashedStr, HashedStr) {
        let item = self.item();
        (self.db.hashes.get_hash(item.path), self.db.hashes.get_hash(item.language), self.db.hashes.get_hash(item.extension) )
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

    pub fn children(&'a self) -> ChildIterator<'a> {
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

    pub fn data_len(&self) -> usize {
        let item = self.item();
        match &item.specifics {
            ItemRecordSpecifics::Folder(_) => 0,
            ItemRecordSpecifics::File(fi) => {
                let packref = fi.packages.get(0).unwrap();
                let maybe_package = self.db.packages.get(packref.package_number as usize);
                let maybe_packentry = maybe_package.and_then(|p| p.files.get(packref.file_number as usize));
                maybe_packentry.unwrap().length.try_into().unwrap()
            }
        }
    }

    pub fn item_index(&self) -> u32 { self.item_number }

    pub fn get_backing_details(&self) -> Option<(&'a Path, usize, usize)> {
        let item = self.item();
        match &item.specifics {
            ItemRecordSpecifics::Folder(_) => None,
            ItemRecordSpecifics::File(fi) => {
                let packref = fi.packages.get(0).unwrap();
                let package = self.db.packages.get(packref.package_number as usize).unwrap();
                let packentry = package.files.get(packref.file_number as usize).unwrap();
                return Some((&package.data_path, packentry.offset as usize, packentry.length as usize));
            }
        }
    }
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

pub fn from_bdb<'a>(mut hashlist: HashIndex, bdb: &bundledb_reader::BundleDbFile, packages: &Vec<loader::ParsedBundle>) -> Database {
    println!("{:?} from_bdb() start", SystemTime::now());
    let mut items = Vec::<ItemRecord>::new();
    let mut itemkeys_by_file_id = FnvHashMap::<u32, (u64, u64, u64)>::default();
    let mut folder_paths = FnvHashSet::<u64>::default();
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
        let path = match hs.text {
            None => String::new(),
            Some(t) => t.to_owned()
        };
        for (i, c) in path.char_indices() {
            if c == '/' {
                let h = hashlist.intern_substring(bdbe.path, 0..i);
                folder_paths.insert(h.unwrap());
            }
        }
    }

    // root folder
    if !folder_paths.contains(&diesel_hash::EMPTY) {
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

    for h in folder_paths.drain() {
        items.push(ItemRecord {
            path: h,
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
            None => PathSortKey(vec![item.path]),
            Some(path) => {
                if path == "" {
                    PathSortKey(vec![])
                }
                else {
                    let components = path.split('/').map(diesel_hash::hash_str).collect();
                    PathSortKey(components)
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

    item_index.insert((diesel_hash::EMPTY,diesel_hash::EMPTY,diesel_hash::EMPTY), 0);

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

    println!("{:?} from_bdb() end", SystemTime::now());
    Database {
        hashes: Arc::new(hashlist),
        item_index,
        items,
        packages: package_catalog,
        package_index
    }
}

#[derive(PartialEq, Eq)]
struct PathSortKey(Vec<u64>);

impl PartialOrd<PathSortKey> for PathSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for PathSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.len().cmp(&other.0.len()) {
            Ordering::Equal => self.0.cmp(&other.0),
            c => c
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