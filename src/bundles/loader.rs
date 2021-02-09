use std::collections::HashMap;
use std::fs;
use std::ffi::OsStr;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::diesel_hash;

use super::bundledb_reader;
use super::packageheader_reader;
use super::ReadError;

pub fn load_bundle_dir(dir: &Path) -> Result<(bundledb_reader::BundleDbFile, Vec<ParsedBundle>), ReadError> {
    let bdb_path = dir.join("bundle_db.blb");
    let bdb_data = fs::read(bdb_path)?;
    let bdb = bundledb_reader::read_bundle_db(&bdb_data);

    let bundle_paths = collect_bundle_files(&dir)?;

    let mut multi_headers = HashMap::<PathBuf, packageheader_reader::MultiBundleHeader>::new();
    let mut bundle_headers = Vec::<ParsedBundle>::new();
    
    for fi in bundle_paths {
        let data_stat = fs::metadata(&fi.data_path)?;
        let header_stat = fs::metadata(&fi.header_path)?;
        
        let data_mtime = data_stat.modified()?;
        let header_mtime = header_stat.modified()?;
        let last_modified = if data_mtime > header_mtime { data_mtime } else { header_mtime };

        let header: packageheader_reader::PackageHeaderFile;

        match fi.multi_header_index {
            None => {
                let bundle_bytes = fs::read(&fi.header_path)?;
                header = packageheader_reader::read_normal(&bundle_bytes, data_stat.len())?;
            },
            Some(idx) => {
                if !multi_headers.contains_key(&fi.header_path) {
                    let headers_bytes = fs::read(&fi.header_path)?;
                    let multi_header = packageheader_reader::read_multi(&headers_bytes)?;
                    multi_headers.insert(PathBuf::from(&fi.header_path), multi_header);
                }
                let mh = multi_headers.get(&fi.header_path).unwrap();
                let header_maybe = mh.bundles.get(&idx);
                match header_maybe {
                    None => return Err(ReadError::BadMultiBundleHeader),
                    Some(h) => header = h.clone()
                }
            }
        }

        bundle_headers.push(ParsedBundle {
            data_path: fi.data_path,
            last_modified,
            package_id: fi.package_id,
            header
        });
    }

    return Ok((bdb, bundle_headers));
}

pub struct ParsedBundle {
    pub data_path: PathBuf,
    pub last_modified: SystemTime,
    pub package_id: u64,
    pub header: packageheader_reader::PackageHeaderFile
}

struct BundleFileInfo {
    data_path: PathBuf,
    header_path: PathBuf,
    multi_header_index: Option<u64>,
    package_id: u64
}

fn collect_bundle_files(dir: &Path) -> Result<Vec<BundleFileInfo>, IoError> {
    let mut result : Vec<BundleFileInfo> = Vec::new();

    let dirents_iter = fs::read_dir(&dir)?;
    for dirent_r in dirents_iter {
        match dirent_r {
            Err(e) => return Err(e),
            Ok(dirent) => if let Some(bfi) = file_info_for_dirent(&dirent) { result.push(bfi) }
        }
    }
    return Ok(result);
}

fn file_info_for_dirent(dirent: &fs::DirEntry) -> Option<BundleFileInfo> {
    let data_path = dirent.path();
    let header_path : PathBuf;
    let multi_header_index: Option<u64>;
    let package_id: u64;

    if data_path.extension().and_then(OsStr::to_str) != Some("bundle") {
        return None;
    }
   
    match data_path.file_stem().and_then(OsStr::to_str) {
        None => return None,
        Some(stem) => {
            let underscore_split: Vec<&str> = stem.splitn(2, '_').collect();
            if underscore_split.len() == 2 {
                if underscore_split[1] == "h" {
                    return None;
                }
                else {
                    header_path = data_path.with_file_name(format!("{}_h.bundle",underscore_split[0]));
                    multi_header_index = Some(underscore_split[1].parse::<u64>().unwrap());
                    package_id = diesel_hash::hash_str(stem);
                }
            }
            else {
                header_path = data_path.with_file_name(format!("{}_h.bundle",underscore_split[0]));
                multi_header_index = None;
                package_id = u64::from_str_radix(underscore_split[0], 16).unwrap().swap_bytes();
            }
        }
    }

    let header_meta_r = std::fs::metadata(&header_path);
    if let Err(e) = header_meta_r {
        if e.kind() == std::io::ErrorKind::NotFound {
            println!("404: {:?} {:?}", header_path, data_path);
            return None;
        }
        else {
            panic!("IO error locating bundle header for {:?}: {}", data_path, e);
        }
    }
    return Some(BundleFileInfo {
        data_path,
        header_path,
        multi_header_index,
        package_id
    });
}