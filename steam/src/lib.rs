use std::path::{Path, PathBuf};

use libraryfolders::LibraryFolders;
use thiserror::Error;

pub mod vdf;
mod libraryfolders;

#[cfg(windows)]
pub fn steam_directory() -> Result<String, Error> {
    use registry::{Hive, Security, Data};
    let key = Hive::CurrentUser.open(r"SOFTWARE\Valve\Steam", Security::Read)
        .map_err(Into::<registry::Error>::into)?;
    let pathdata = key.value("SteamPath")
        .map_err(Into::<registry::Error>::into)?;
    match pathdata {
        Data::String(s) => {
            s.to_string().map_err(|_| Error::BadSteamPath)
        }
        _ => Err(Error::BadSteamPath)
    }
}

#[cfg(not(windows))]
pub fn steam_directory() -> Result<String, Error> {
    Error::SteamLookupUnimplemented
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Only know how to find Steam on Windows at present")]
    SteamLookupUnimplemented,

    #[cfg(windows)]
    #[error("Couldn't look Steam's path up in the registry: {0:?}")]
    SteamRegistryLookupFailed(#[from] registry::Error),

    #[cfg(not(windows))]
    #[error("How did you even get here, the Registry is a Windows thing: {0:?}")]
    SteamRegistryLookupFailed(Box<dyn Error>),

    #[error("Steam's path missing or too mangled to turn to string")]
    BadSteamPath,

    #[error("IO error reading {0:?}: {1:?}")]
    IoError(PathBuf, std::io::Error),

    #[error("{0:?} is not UTF-8: {1:?}")]
    BadEncoding(PathBuf, std::string::FromUtf8Error),

    #[error("Failed to parse {0:?}: {1:?}")]
    BadVdfParse(PathBuf, vdf::Error),

    #[error("Parsed libraryfolders.vdf but the schema is unrecognised")]
    BadLibraryFoldersSchema,

    #[error("Unrecognised appmanifest schema")]
    BadAppmanifestSchema,

    #[error("Game with id {0} not detected")]
    GameNotDetected(String)
}

pub fn try_get_app_directory(appid: &str) -> Result<PathBuf, Error> {
    let steamdir = steam_directory()?;

    let mut vdfpath = std::path::PathBuf::from(steamdir);
    vdfpath.push("steamapps");
    vdfpath.push("libraryfolders.vdf");

    
    let vdf = read_vdf(&vdfpath)?;

    let mut relevant_folder: PathBuf = LibraryFolders::from_vdf(&vdf)?
        .libraries
        .iter()
        .filter(|lib| lib.app_ids.iter().any(|i| i == appid))
        .map(|lib| lib.path.clone())
        .next()
        .ok_or_else(|| Error::GameNotDetected(appid.into()))?
        .into();
    
    relevant_folder.push("steamapps");
    
    let mut acfpath = relevant_folder.clone();
    acfpath.push(format!("appmanifest_{}.acf", appid));
    let vdf = read_vdf(&acfpath)?;

    let install_folder = vdf
        .has_name("AppState").ok_or(Error::BadAppmanifestSchema)?
        .section_data().ok_or(Error::BadAppmanifestSchema)?
        .iter()
        .filter(|i| i.name == "installdir")
        .flat_map(vdf::Node::string_data)
        .next().ok_or(Error::BadAppmanifestSchema)?;

    relevant_folder.push("common");
    relevant_folder.push(install_folder);
    Ok(relevant_folder)
}

fn read_vdf(file: &Path) -> Result<vdf::Node, Error> {
    let bytes = std::fs::read(file).map_err(|e| Error::IoError(file.to_owned(), e) )?;
    let text = String::from_utf8(bytes).map_err(|e| Error::BadEncoding(file.to_owned(), e))?;
    vdf::parse(&text).map_err(|e| Error::BadVdfParse(file.to_owned(), e))
}