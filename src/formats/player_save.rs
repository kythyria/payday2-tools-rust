//! Player save data
//!
//! This is partly a direct port of https://github.com/rohvani/PD2SE

use std::collections::HashMap;
use anyhow::{Context, Result, bail, ensure};
use itertools::Itertools;
use crate::util::binaryreader::*;

#[derive(Debug)]
pub struct SaveData {
    pub head: Vec<u8>,
    pub body: DataItem,
    pub foot: Vec<u8>,
}

#[derive(Debug)]
pub enum DataItem {
    String(String),
    ScrambledString(Vec<u8>),
    Float(f32),
    Empty, // Might be zero!
    Byte(u8),
    Short(u16),
    Bool(bool),
    Dictionary(HashMap<DataItem, DataItem>),
    Unknown9([u8; 12])
}
impl PartialEq for DataItem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::ScrambledString(l0), Self::ScrambledString(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => {
                l0.is_nan() && r0.is_nan() || l0 == r0
            },
            (Self::Byte(l0), Self::Byte(r0)) => l0 == r0,
            (Self::Short(l0), Self::Short(r0)) => l0 == r0,
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Dictionary(l0), Self::Dictionary(r0)) => l0 == r0,
            (Self::Unknown9(l0), Self::Unknown9(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
impl Eq for DataItem {}
impl std::hash::Hash for DataItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            DataItem::String(s) => s.hash(state),
            DataItem::ScrambledString(s) => s.hash(state),
            DataItem::Float(f) => {
                if f.is_nan() { 
                    f32::NAN.to_ne_bytes().hash(state)
                }
                else {
                    f.to_ne_bytes().hash(state)
                }
            },
            DataItem::Empty => (),
            DataItem::Byte(b) => b.hash(state),
            DataItem::Short(s) => s.hash(state),
            DataItem::Bool(b) => b.hash(state),
            DataItem::Dictionary(dict) => {
                dict.len().hash(state);
                for (k,v) in dict {
                    k.hash(state);
                    v.hash(state);
                }
            },
            DataItem::Unknown9(b) => b.hash(state)
        }
    }
}

const XOR_KEY: &[u8] = &[
    0x74, 0x3E, 0x3F, 0xA4, 0x32, 0x43, 0x26, 0x2E,0x23, 0x36, 
    0x37, 0x6A, 0x6D, 0x3A, 0x48, 0x47, 0x3D, 0x53, 0x2D, 0x63, 
    0x41, 0x6B, 0x29, 0x38, 0x6A, 0x68, 0x5F, 0x4D, 0x4A, 0x68, 
    0x3C, 0x6E, 0x66, 0xF6
];

/// XOR-based symmetric scramble/descramble
pub fn scramble(scrambled_data: &[u8]) -> Vec<u8> {
    let mut data = scrambled_data.to_owned();
    for i in 0..data.len() {
        let key_idx = data.len().wrapping_add(i).wrapping_mul(7).wrapping_rem(XOR_KEY.len());
        let key: u64 = XOR_KEY[key_idx].try_into().unwrap();
        let ctr: u64 = (data.len() - i).try_into().unwrap();
        let scramble: u8 = key.wrapping_mul(ctr) as u8;
        data[i] ^= scramble;
    }
    data
}

pub fn parse(data: &[u8]) -> Result<SaveData> {
    //let unscrambled = scramble(data);
    let mut cursor: &[u8] = data.as_ref();

    let version: u32 = cursor.read_item().context("Failed to read version (empty input?)")?;
    ensure!(version == 10, "Unknown SaveData version {}", version);

    let head = read_datablock(&mut cursor).context("Failed reading head")?;
    let body_bytes = read_datablock(&mut cursor).context("Failed reading body")?;
    let body = read_item(&mut body_bytes.as_ref()).context("Failed decoding body")?;
    let foot = read_datablock(&mut cursor).context("Failed reading foot")?;

    Ok(SaveData {head, body, foot})
}

fn read_datablock(bytes: &mut &[u8]) -> Result<Vec<u8>> {
    let block_size: u32 = bytes.read_item().context("Failed reading block size")?;
    let block_version: u32 = bytes.read_item().context("Failed reading block version")?;
    ensure!(block_version == 10, "Unknown datablock version");
    let body_size = block_size - 16 - 4; // 16 bytes of checksum at the end, 4 bytes of size, length doesn't count
    let (body, rest) = bytes.split_at(body_size as usize);
    let (_checksum, rest) = rest.split_at(16);
    *bytes = rest;
    Ok(body.to_owned())
}

fn read_item(bytes: &mut &[u8]) -> Result<DataItem> {
    let item_addr = bytes.as_ptr();
    let tag: u8 = bytes.read_item().context("Failed to read tag")?;
    let res = match tag {
        1 => read_string(bytes).context("Failed to read string")?,
        2 => DataItem::Float(bytes.read_item::<f32>().context("Failed to read float")?),
        3 => DataItem::Empty,
        4 => DataItem::Byte(bytes.read_item::<u8>().context("Failed to read byte")?),
        5 => DataItem::Short(bytes.read_item::<u16>().context("Failed to read short")?),
        6 => DataItem::Bool(bytes.read_item::<bool>().context("Failed to read bool")?),
        7 => DataItem::Dictionary(read_dictionary(bytes).context(format!("Failed to read dictionary @ {:p}", item_addr))?),
        9 => DataItem::Unknown9(bytes.read_item().context(format!("Failed to read Unknown9 @ {:p}", item_addr))?),
        _ => bail!("Invalid tag {} @ {:p}", tag, item_addr)
    };
    Ok(res)
}

const STRING_PADDING: &[u8] = &[ 0xDF, 0xC1, 0xA3, 0x85, 0x67, 0x49, 0x2B, 0x0D, 0xED, 0xCF, 0xB1, 0x93 ];
/*
If rohvani's code is correct, which I'm assuming it is, digested values are always an even number of bytes.
xx DF is invalid UTF-8 (expects 2 trailers)
xx DF xx C1 is also invalid (C1 is not a trailer and expects two continuation bytes.)
xx DF xx C1 xx A3 is still invalid because C1 isn't a trailer.
So it's always invalid, woot.
 */


fn read_string(bytes: &mut &[u8]) -> Result<DataItem> {
    let mut buf = Vec::<u8>::new();
    loop {
        ensure!(bytes.len() > 0, "String not terminated!");
        let byte: u8 = bytes.read_item()?;
        if byte == 0 { break; }
        buf.push(byte);
    }

    let buf = match String::from_utf8(buf) {
        Ok(s) => return Ok(DataItem::String(s)),
        Err(e) => e.into_bytes()
    };

    let pairs: &[[u8; 2]] = match bytemuck::try_cast_slice(buf.as_slice()) {
        Ok(o) => o,
        Err(_) => {
            bail!("String is neither valid UTF-8 nor valid digest (odd length): [{:x}]",buf.iter().format(" "))
        }
    };

    let mut descrambled = Vec::<u8>::with_capacity(pairs.len());
    for (idx, [byte, pad]) in pairs.into_iter().enumerate() {
        if *pad != STRING_PADDING[idx] {
            bail!("String is invalid digest (bad padding): [{:x}]",buf.iter().format(" "));
        }
        descrambled.push(0xFEu8.wrapping_sub(*byte));
    }
    Ok(DataItem::ScrambledString(descrambled))
}

fn read_dictionary(bytes: &mut &[u8]) -> Result<HashMap<DataItem, DataItem>> {
    let len: u32 = bytes.read_item().context("Failed to read dictionary length")?;
    let mut res = HashMap::with_capacity(len as usize);
    for _ in 0..len {
        let key = read_item(bytes).context("Failed to read dict key")?;
        let value = read_item(bytes).context("Failed to read dict value")?;
        res.insert(key, value);
    }
    Ok(res)
}

impl From<&SaveData> for crate::notation_rs::Item {
    fn from(value: &SaveData) -> Self {
        use crate::notation_rs::*;

        let mut c = Compound::new_braced().with_tag("SaveData");

        c.push_bare("head", Item::new_binary(&value.head));
        c.push_bare("body", (&value.body).into());
        c.push_bare("foot", Item::new_binary(&value.foot));

        Item::Compound(c)
    }
}

impl From<&DataItem> for crate::notation_rs::Item {
    fn from(value: &DataItem) -> Self {
        use crate::notation_rs::*;
        match value {
            DataItem::String(s) => Item::new_string(s),
            DataItem::ScrambledString(s) => {
                let mut c = Compound::new_parenthesized().with_tag("Scramble");
                c.push_indexed(Item::new_binary(&*s));
                Item::Compound(c)
            },
            DataItem::Float(f) => Item::new_float(*f),
            //DataItem::Empty => Item::new_bare("none"),
            DataItem::Empty => Item::new_integer(0),
            DataItem::Byte(b) => Item::new_u8(*b),
            DataItem::Short(s) => Item::new_u16(*s),
            DataItem::Bool(true) => Item::new_bare("true"),
            DataItem::Bool(false) => Item::new_bare("false"),
            DataItem::Dictionary(dict) => {
                let mut c = Compound::new_braced();
                for (k, v) in dict.iter() {
                    c.push(k.into(), v.into());
                }
                Item::Compound(c)
            },
            DataItem::Unknown9(b) => {
                let mut c = Compound::new_parenthesized().with_tag("Unknown9");
                c.push_indexed(Item::new_binary(&*b));
                Item::Compound(c)
            }
        }
    }
}