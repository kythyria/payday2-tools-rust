use std::io::{ErrorKind, Error as IoError};
use std::fmt::Write;

use rayon::prelude::*;

use crate::bundles::database::Database;
use crate::diesel_hash::{hash_str as dhash};
use crate::formats::banksinfo;
use crate::util::BoxResult as Result;

pub fn scan(database: &Database) -> Result<Vec<Box<str>>> {

    let banksinfo = get_banks_info(database)?;

    let bm = banksinfo.sound_lookups.values().map(|(h, s)| (h.0, s.as_ref()));
    // copies, but it shuts up the borrow checker.
    let lookups_ref: Vec<(u64, &str)> = bm.collect();
    let banks_ref: Vec<&str> = banksinfo.sound_banks.iter().map(|i| i.as_ref()).collect();
    
    let longest_bank = banks_ref.iter().map(|i| i.len()).max().unwrap_or_default();
    let longest_lookup_name = lookups_ref.iter().map(|(_,s)| s.len()).max().unwrap_or_default();
    let str_capacity = "/streamed/".len() + longest_bank + longest_lookup_name;

    let pr: Vec<Box<str>> = lookups_ref.par_iter().filter_map(|(path_hash, filename)| {
        let mut buf = String::with_capacity(str_capacity);
        for bank_name in banks_ref.iter() {
            buf.clear();
            let slashidx = bank_name.rfind('/').unwrap();
            let (left, right) = bank_name.split_at(slashidx);
            write!(buf, "{}/streamed{}/{}", left, right, filename).unwrap(); // how can this even fail, it's big enough. We checked up there.
            if dhash(&buf) == *path_hash {
                return Some(Box::from(buf));
            }
        }
        None
    }).collect();
    return Ok(pr);
}

fn get_banks_info(database: &Database) -> Result<banksinfo::BanksInfo> {
    let item = database.get_by_str("existing_banks", "", "banksinfo")
        .ok_or(IoError::new(ErrorKind::NotFound, "No existing_banks.banksinfo!"))?;
    
    let bytes = item.read_data()?;
    Ok(banksinfo::try_from_bytes(&bytes)?)
}