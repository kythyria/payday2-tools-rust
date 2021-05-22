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
use nom::combinator::map;
use nom::multi::length_count;
use nom::number::complete::le_u32;
use nom::sequence::tuple;

use vek::{Rgb, Vec2, Vec3};

use crate::util::read_helpers::{TryFromIndexedLE, TryFromBytesError};
use crate::util::parse_helpers;
use crate::util::parse_helpers::Parse;
use pd2tools_macros::{EnumTryFrom, Parse};

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

#[derive(Debug, Parse)]
struct Anim3 {
    start_time: f64,
    end_time: f64,

    author_tag: String,
    source: String,
    scene_type: String,
}

#[derive(Debug, Parse)]
struct Material {
    id: u32,
    name: String,
    parent_id: u32
}

#[derive(Debug, Parse)]
struct Node {
    id: u32,
    name: String,

    transform: vek::Mat4<f64>,
    pivot_transform: vek::Mat4<f64>,

    parent_id: u32
}

#[derive(Debug, Parse)]
struct Geometry {
    node_id: u32,

    /// ID of mesh material
    /// 0xFFFFFFFF == none
    material_id: u32,
    unknown1: u16,
    channels: Vec<GeometryChannel>,
    faces: Vec<GeometryFace>
}

#[derive(Debug)]
enum GeometryChannel {
    Position(u32, Vec<Vec3<f64>>),
    Normal  (u32, Vec<Vec3<f64>>),
    Binormal(u32, Vec<Vec3<f64>>),
    Tangent (u32, Vec<Vec3<f64>>),
    TexCoord(u32, Vec<Vec2<f64>>),
    Colour  (u32, Vec<Rgb<f64>>)
}
impl Parse for GeometryChannel {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (input, (kind, layer)) = tuple((le_u32, le_u32))(input)?;
        let col3d = Rgb::<f64>::parse;
        let vec3d = Vec3::<f64>::parse;
        let vec2d = Vec2::<f64>::parse;
        match kind {
            0x00000000 => map(length_count(le_u32, vec3d), |v| GeometryChannel::Position(layer, v))(input),
            0x00000001 => map(length_count(le_u32, vec2d), |v| GeometryChannel::TexCoord(layer, v))(input),
            0x00000002 => map(length_count(le_u32, vec3d), |v| GeometryChannel::Normal(layer, v))(input),
            0x00000003 => map(length_count(le_u32, vec3d), |v| GeometryChannel::Binormal(layer, v))(input),
            0x00000004 => map(length_count(le_u32, vec3d), |v| GeometryChannel::Tangent(layer, v))(input),
            0x00000005 => map(length_count(le_u32, col3d), |v| GeometryChannel::Colour(layer, v))(input),
            _ => {
                println!("Unknown geometry kind {:016x}", kind);
                Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::OneOf)))
            }
        }
    }

    fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
        fn write_variant<T: Parse, O: std::io::Write>(output: &mut O, discriminant: u32, layer: &u32, data: &T) -> std::io::Result<()> {
            discriminant.serialize(output)?;
            layer.serialize(output)?;
            data.serialize(output)
        }

        match self {
            GeometryChannel::Position(layer, data) => write_variant(output, 0, layer, data),
            GeometryChannel::TexCoord(layer, data) => write_variant(output, 1, layer, data),
            GeometryChannel::Normal(layer, data) => write_variant(output, 2, layer, data),
            GeometryChannel::Binormal(layer, data) => write_variant(output, 3, layer, data),
            GeometryChannel::Tangent(layer, data) => write_variant(output, 4, layer, data),
            GeometryChannel::Colour(layer, data) => write_variant(output, 5, layer, data)
        }
    }
}

#[derive(Debug, Parse)]
struct GeometryFace {
    material_id: u32,
    unknown1: u32,

    loops: Vec<GeometryFaceloop>
}

#[derive(Debug, Parse)]
struct GeometryFaceloop {
    channel: u32,
    a: u32,
    b: u32,
    c: u32
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumTryFrom, Parse)]
#[repr(u32)]
enum LightType {
    Spot = 0,
    Directional = 1,
    Omni = 2
}

#[derive(Debug, PartialEq, Clone, Copy, EnumTryFrom, Parse)]
#[repr(u32)]
enum SpotlightShape {
    Rectangular = 0,
    Circular = 1
}

#[derive(Debug, Parse)]
struct Light {
    node_id: u32,
    lamp_type: LightType,
    color: Rgb<f64>,
    multiplier: f64,
    attenuation_end: f64,
    attenuation_start: f64,
    unknown_2: f64,
    unknown_3: f64,
    falloff: f64,
    hotspot: f64,
    aspect_ratio: f64,
    overshoot: bool,
    shape: SpotlightShape,
    target: u32,
    on: bool
}

/// "Beats and triggers" block.
struct Unknown21 {
    unknown_1: u32, // probably a count of unknown_2
    unknown_2: Vec<Unknown21Item>
}

#[derive(Parse)]
struct Unknown21Item {
    unknown_1: u32,
    unknown_2: String,
    unknown_3: f64,
    unknown_4: u32,    // The maya2017 exporter always writes 0xFFFFFFFF,
    unknown_5: String, // Exporter always writes "beat" or "trigger" here
    unknown_6: u32     // Exporter always writes 0
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
        let r#type = TypeId::try_from(sec.type_code);
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