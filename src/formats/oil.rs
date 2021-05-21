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

use std::{convert::TryFrom, path::Path};

use nom::IResult;
use nom::bytes::complete::take;
use nom::combinator::{map_res, map};
use nom::multi::{length_data, length_count};
use nom::number::complete::{le_u16, le_u32, le_f64, le_u64};
use nom::sequence::tuple;
use nom::multi::fill;
use nom_derive::{NomLE, Parse};

use crate::util::read_helpers::{TryFromIndexedLE, TryFromBytesError};
use crate::util::parse_helpers;
use pd2tools_macros::EnumTryFrom;

struct UnparsedSection<'a> {
    type_code: u32,
    length: usize,
    offset: usize,
    bytes: &'a [u8]
}

#[derive(Debug, EnumTryFrom)]
#[repr(u32)]
enum TypeId {
    Node = 0,
    Anim = 3,
    Material = 4,
    Geometry = 5,
    Anim2 = 12,
    Anim3 = 20,
    Light = 10,
    UnknownType11 = 11, // this seems to be a single counted block.
    UnknownType21 = 21 // mentioned next to a log message about "beats and triggers"
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

/// Generate a parse_le method for things only having parse.
///
/// `nom_derive::NomLE` doesn't generate parse_le for structs for some reason.
macro_rules! le_shim {
    ($t:ty) => {
        impl $t {
            pub fn parse_le <'nom>(orig_i: &'nom [u8]) -> nom::IResult<&'nom [u8], Self>
            {
                Self::parse(orig_i)
            }
        }
    }
}

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

#[derive(Debug, NomLE)]
#[nom(Complete)]
struct Anim3 {
    start_time: f64,
    end_time: f64,

    #[nom(Parse="prefixed_string")] author_tag: String,
    #[nom(Parse="prefixed_string")] source: String,
    #[nom(Parse="prefixed_string")] scene_type: String,
}
le_shim!(Anim3);

#[derive(Debug, NomLE)]
#[nom(Complete)]
struct Material {
    id: u32,

    #[nom(Parse="prefixed_string")]
    name: String,

    parent_id: u32
}

#[derive(Debug, NomLE)]
#[nom(Complete)]
struct Node {
    id: u32,
    name: String,

    #[nom(Parse="matrix_4x4")] transform: [f64; 16],
    #[nom(Parse="matrix_4x4")] pivot_transform: [f64; 16],

    parent_id: u32,
    trailing_unparsed: Vec<u8>
}
le_shim!(Node);

#[derive(Debug, NomLE)]
#[nom(Complete)]
struct Geometry {
    node_id: u32,

    /// ID of mesh material
    /// 0xFFFFFFFF == none
    material_id: u32,
    unknown1: u16,
    #[nom(LengthCount="le_u32")] channels: Vec<GeometryChannel>,
    #[nom(LengthCount="le_u32")] faces: Vec<GeometryFace>
}
le_shim!(Geometry);

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
    fn parse_le<'a>(value: &'a[u8]) -> IResult<&'a[u8], Self> {
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

#[derive(Debug, NomLE)]
struct GeometryFace {
    material_id: u32,
    unknown1: u32,

    #[nom(LengthCount="le_u32")]
    loops: Vec<GeometryFaceloop>
}
le_shim!(GeometryFace);

#[derive(Debug, NomLE)]
struct GeometryFaceloop {
    channel: u32,
    a: u32,
    b: u32,
    c: u32
}
le_shim!(GeometryFaceloop);

#[derive(Debug, PartialEq, Eq, NomLE)]
#[repr(u32)]
enum LightType {
    Spot = 0,
    Directional = 1,
    Omni = 2
}
le_shim!(LightType);

#[derive(Debug, PartialEq, NomLE)]
#[repr(u32)]
enum SpotlightShape {
    Rectangular = 0,
    Circular = 1
}
le_shim!(SpotlightShape);

#[derive(Debug, NomLE)]
struct LightColor {
    pub r: f64,
    pub g: f64,
    pub b: f64
}
le_shim!(LightColor);

#[derive(Debug, NomLE)]
#[nom(Complete)]
struct Light {
    node_id: u32,
    lamp_type: LightType, // Given the position, probably type (0=spot, 1=directional, 2=omni/point)
    color: LightColor,
    multiplier: f64,
    attenuation_end: f64,
    attenuation_start: f64,
    unknown_2: f64,
    unknown_3: f64,
    falloff: f64,
    hotspot: f64,
    aspect_ratio: f64,

    #[nom(Parse="bool_u8")]
    overshoot: bool,

    shape: SpotlightShape,
    target: u32,

    #[nom(Parse="bool_u8")]
    on: bool
}

/// "Beats and triggers" block.
struct Unknown21 {
    unknown_1: u32, // probably a count of unknown_2
    unknown_2: Vec<Unknown21Item>
}

struct Unknown21Item {
    unknown_1: u32,
    unknown_2: String,
    unknown_3: f64,
    unknown_4: u32,    // The maya2017 exporter always writes 0xFFFFFFFF,
    unknown_5: String, // Exporter always writes "beat" or "trigger" here
    unknown_6: u32     // Exporter always writes 0
}

fn bool_u8<'a>(value: &'a[u8]) -> IResult<&'a [u8], bool> {
    map(nom::bytes::complete::take(1usize), |v: &[u8]| match v[0] {
        0 => false,
        _ => true
    })(value)
}

fn prefixed_string<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], String> {
    map_res(length_data(le_u32), |i: &[u8]| -> Result<String, std::str::Utf8Error> {
        let st = std::str::from_utf8(i)?;
        Ok(String::from(st))
    })(input)
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
        let r#type = TypeId::try_from(sec.type_code as usize);
        match r#type {
            Ok(TypeId::Node) => println!("{:6} {:6} {:?}", sec.offset, sec.length, Node::parse(sec.bytes)),
            Ok(TypeId::Material) => println!("{:6} {:6} {:?}", sec.offset, sec.length, Material::parse(sec.bytes)),
            Ok(TypeId::Geometry) => println!("{:6} {:6} {:?}", sec.offset, sec.length, Geometry::parse(sec.bytes)),
            Ok(TypeId::Light) => println!("{:6} {:6} {:?}", sec.offset, sec.length, Light::parse(sec.bytes)),
            Ok(TypeId::Anim3) => println!("{:6} {:6} {:?}", sec.offset, sec.length, Anim3::parse(sec.bytes)),
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