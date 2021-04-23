//! Reads the OIL intermediate representation produced by Overkill/LGL's exporters
//!
//! According to the deserialiser in Overkill's tools, the format is:
//! ```rs
//! struct Oil {
//!     magic: b"FORM",
//!     total_size: u32,
//!     nodes: [Node]
//! }
//! 
//! struct Node {
//!     type_code: u32,
//!     length: u32,
//!     data: [u8]
//! }
//! ```

use std::path::Path;

use nom::IResult;
use nom::bytes::complete::take;
use nom::combinator::map_res;
use nom::multi::length_data;
use nom::number::complete::{le_u32, le_f64};
use nom::sequence::tuple;
use nom::multi::fill;

use crate::util::read_helpers::*;

struct UnparsedSection<'a> {
    type_code: u32,
    length: usize,
    offset: usize,
    bytes: &'a [u8]
}

#[derive(Debug)]
#[repr(u32)]
enum TypeId {
    Node = 0,
    Anim = 3,
    Material = 4,
    Anim2 = 12,
    Anim3 = 20,
    UnknownType11 = 11,
    UnknownType5 = 5,
    UnknownType21 = 21
}

#[derive(Debug)]
enum ParseError {
    NoMagic,
    UnexpectedEof,
    BadUtf8,
    SectionTooShort,
    Mysterious
}
macro_rules! trivialer_from {
    ($type:ty, $variant:ident) => {
        impl From<$type> for ParseError {
            fn from(_: $type) -> Self {
                ParseError::$variant
            }
        }
    }
}
trivialer_from!(TryFromBytesError, UnexpectedEof);
trivialer_from!(std::str::Utf8Error, BadUtf8);

fn mysterious_err<E>(error: nom::Err<E>) -> nom::Err<ParseError> {
    match error {
        nom::Err::Incomplete(_) => nom::Err::Failure(ParseError::UnexpectedEof),
        nom::Err::Error(e) => nom::Err::Error(ParseError::Mysterious),
        nom::Err::Failure(_) => nom::Err::Failure(ParseError::Mysterious)
    }
}

#[derive(Debug)]
struct Anim3 {
    start_time: f64,
    end_time: f64,
    author_tag: String,
    source: String,
    scene_type: String,
    trailing_unparsed: Vec<u8>
}
impl Anim3 {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Anim3> {
        let mut tup = tuple((le_f64, le_f64, prefixed_string, prefixed_string, prefixed_string));
        let (remaining, (start_time, end_time, author_tag, source, scene_type)) = tup(value)?;
        
        Ok((b"", Anim3 {
            start_time,
            end_time,
            author_tag: author_tag.into(),
            source: source.into(),
            scene_type: scene_type.into(),
            trailing_unparsed: remaining.to_owned()
        }))
    }
}

#[derive(Debug)]
struct Material {
    id: u32,
    name: String,
    parent_id: u32,
    trailing_unparsed: Vec<u8>
}

impl Material {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Material> {
        let mut tup = tuple((le_u32, prefixed_string, le_u32));
        let (remaining, (id, name, parent_id)) = tup(value)?;

        Ok((b"", Material {
            id,
            name: name.into(),
            parent_id,
            trailing_unparsed: remaining.to_owned()
        }))
    }
}

#[derive(Debug)]
struct Node {
    id: u32,
    name: String,
    transform: [f64; 16],
    pivot_transform: [f64; 16],
    parent_id: u32,
    trailing_unparsed: Vec<u8>
}

impl Node {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Node> {
        let mut tup = tuple((le_u32, prefixed_string, matrix_4x4, matrix_4x4, le_u32));
        let (remaining, (id, name, transform, pivot_transform, parent_id)) = tup(value)?;

        Ok((b"", Node {
            id,
            name: name.to_owned(),
            transform,
            pivot_transform,
            parent_id,
            trailing_unparsed: remaining.to_owned()
        }))
    }
}

fn read_prefixed_string<'a>(src: &'a [u8], offset: usize) -> Result<(&'a str, usize), ParseError> {
    let strlen = u32::try_from_le(src, offset)? as usize;
    let start = offset + 4;
    let end = start + strlen;
    if start >= src.len() || end > src.len() {
        return Err(ParseError::UnexpectedEof)
    }
    let bytes = &src[start..end];
    Ok((std::str::from_utf8(bytes)?, end))
}


fn prefixed_string<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], &'a str> {
    map_res(length_data(le_u32), std::str::from_utf8)(input)
}

fn matrix_4x4<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], [f64; 16]> {
    let mut out: [f64; 16] = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
    let (rest, ()) = fill(le_f64, &mut out)(input)?;
    Ok((rest, out))
}

fn split_to_sections<'a>(src: &'a [u8]) -> Result<Vec<UnparsedSection<'a>>, ParseError> {
    let mut out = Vec::<UnparsedSection>::new();

    if src[0..4] != *b"FORM" {
        return Err(ParseError::NoMagic)
    }

    let total_size = match u32::try_from_le(src, 4) {
        Ok(v) => v as usize,
        Err(_) => return Err(ParseError::UnexpectedEof)
    };

    let mut curr_offset: usize = 8;
    while curr_offset - 8 < total_size {
        let type_code = u32::try_from_le(src, curr_offset)?;
        let length = u32::try_from_le(src, curr_offset + 4)? as usize;
        let body_offset = curr_offset + 8;
        if body_offset + length > src.len() {
            return Err(ParseError::UnexpectedEof);
        }

        out.push(UnparsedSection {
            type_code,
            length,
            offset: body_offset,
            bytes: &src[body_offset..(body_offset + length)]
        });

        curr_offset += length + 8;
    }

    Ok(out)
}

pub fn print_sections(filename: &Path) {
    let bytes = match std::fs::read(filename) {
        Err(e) => { println!("Error reading {:?}: {}", filename, e); return} 
        Ok(v) => v
    };
    
    let data = match split_to_sections(&bytes) {
        Err(e) => { println!("Error reading {:?}: {:?}", filename, e); return},
        Ok(v) => v
    };

    for sec in data {
        match sec.type_code {
            0 => println!("{:6} {:6} {:?}", sec.offset, sec.length, Node::parse(sec.bytes)),
            4 => println!("{:6} {:6} {:?}", sec.offset, sec.length, Material::parse(sec.bytes)),
            20 => println!("{:6} {:6} {:?}", sec.offset, sec.length, Anim3::parse(sec.bytes)),
            _ => {
                let slice = if sec.bytes.len() > 64 { &sec.bytes[0..64] } else { sec.bytes };
                println!("{:6} {:6} {:4} {:}", sec.offset, sec.length, sec.type_code, AsHex(slice))
            }
        }
        
    }
}

struct AsHex<'a>(&'a[u8]);
impl<'a> std::fmt::Display for AsHex<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in self.0 {
            write!(f, "{:02x}", i)?;
        };
        Ok(())
    }
}