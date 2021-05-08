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
use nom::combinator::{map_res, map};
use nom::multi::{length_data, length_count};
use nom::number::complete::{le_u16, le_u32, le_f64, le_u64};
use nom::sequence::tuple;
use nom::multi::fill;

use crate::util::read_helpers::{TryFromIndexedLE, TryFromBytesError};

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
    Geometry = 5,
    Anim2 = 12,
    Anim3 = 20,
    UnknownType11 = 11,
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
        nom::Err::Error(_) => nom::Err::Error(ParseError::Mysterious),
        nom::Err::Failure(_) => nom::Err::Failure(ParseError::Mysterious)
    }
}

struct UnparsedBytes(Vec<u8>);
impl std::fmt::Debug for UnparsedBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", self.0.len())?;
        if false && self.0.len() > 64 {
            write!(f, "[{}...]", AsHex(&self.0[0..64]))?;
        }
        else {
            write!(f, "[{}]", AsHex(&self.0))?;
        }
        Ok(())
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

#[derive(Debug)]
struct Geometry {
    node_id: u32,

    /// ID of mesh material
    /// 0xFFFFFFFF == none
    material_id: u32,
    unknown1: u16,
    channels: Vec<GeometryChannel>,
    faces: Vec<GeometryFace>,
    trailing_unparsed: UnparsedBytes
}
impl Geometry {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Self> {
        let (remaining, (node_id, material_id, unknown1)) = tuple((le_u32, le_u32, le_u16))(value)?;
        let (remaining, channels) = length_count(le_u32, GeometryChannel::parse)(remaining)?;
        let (remaining, faces) = length_count(le_u32, GeometryFace::parse)(remaining)?;

        Ok((b"", Geometry {
            node_id,
            material_id,
            unknown1,
            channels,
            faces,
            trailing_unparsed: UnparsedBytes(remaining.to_owned())
        }))
    }
}

#[derive(Debug)]
enum GeometryChannel {
    Position(u32, Vec<(f64, f64, f64)>),
    Normal  (u32, Vec<(f64, f64, f64)>),
    Binormal(u32, Vec<(f64, f64, f64)>),
    Tangent (u32, Vec<(f64, f64, f64)>),
    TexCoord(u32, Vec<(f64, f64)>),
    Colour  (u32, Vec<(f64, f64, f64)>)
}
impl GeometryChannel {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Self> {
        let (remaining, (kind, layer)) = tuple((le_u32, le_u32))(value)?;
        let tup3d = tuple((le_f64, le_f64, le_f64));
        let tup2d = tuple((le_f64, le_f64));
        match kind {
            0x00000000 => map(length_count(le_u32, tup3d), |v| GeometryChannel::Position(layer, v))(remaining),
            0x00000001 => map(length_count(le_u32, tup2d), |v| GeometryChannel::TexCoord(layer, v))(remaining),
            0x00000002 => map(length_count(le_u32, tup3d), |v| GeometryChannel::Normal(layer, v))(remaining),
            0x00000003 => map(length_count(le_u32, tup3d), |v| GeometryChannel::Binormal(layer, v))(remaining),
            0x00000004 => map(length_count(le_u32, tup3d), |v| GeometryChannel::Tangent(layer, v))(remaining),
            0x00000005 => map(length_count(le_u32, tup3d), |v| GeometryChannel::Colour(layer, v))(remaining),
            _ => {
                println!("Unknown geometry kind {:016x}", kind);
                Err(nom::Err::Failure(nom::error::Error::new(remaining, nom::error::ErrorKind::OneOf)))
            }
        }
    }
}

#[derive(Debug)]
struct GeometryFace {
    material_id: u32,
    unknown1: u32,
    loops: Vec<GeometryFaceloop>
}
impl GeometryFace {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Self> {
        let (remaining, (material_id, unknown1, loops)) = tuple((le_u32, le_u32, length_count(le_u32, GeometryFaceloop::parse)))(value)?;
        Ok((remaining, GeometryFace {
            material_id, unknown1, loops
        }))
    }
}

#[derive(Debug)]
struct GeometryFaceloop {
    channel: u32,
    a: u32,
    b: u32,
    c: u32
}
impl GeometryFaceloop {
    fn parse<'a>(value: &'a[u8]) -> IResult<&'a[u8], Self> {
        let (remaining, (channel, a, b, c)) = tuple((le_u32, le_u32, le_u32, le_u32))(value)?;
        Ok((remaining, GeometryFaceloop {
            channel, a, b, c
        }))
    }
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
            5 => println!("{:6} {:6} {:?}", sec.offset, sec.length, Geometry::parse(sec.bytes)),
            20 => println!("{:6} {:6} {:?}", sec.offset, sec.length, Anim3::parse(sec.bytes)),
            _ => {
                println!("{:6} {:6} {:4} {:}", sec.offset, sec.length, sec.type_code, AsHex(sec.bytes))
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