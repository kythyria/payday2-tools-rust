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

use crate::util::AsHex;
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

macro_rules! make_chunks {
    ($($name:ident = $tag:literal),+) => {
        #[derive(Debug, EnumTryFrom)]
        #[repr(u32)]
        pub enum ChunkId {
            $($name = $tag),+
        }

        pub enum Chunk {
            $($name($name)),+
        }

        impl std::fmt::Debug for Chunk {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match self {
                    $(Self::$name(d) => <$name as std::fmt::Debug>::fmt(d, f)),+
                }
            }
        }
        
        impl<'a> UnparsedSection<'a> {
            fn try_into_chunk(&self) -> IResult<&'a [u8], Chunk> {
                let r#type = ChunkId::try_from(self.type_code);
                match r#type {
                    $(Ok(ChunkId::$name) => {
                        let (remain, output) =  $name::parse(self.bytes)?;
                        Ok((remain, Chunk::$name(output)))
                    }),+
                    Err(_) => Err(nom::Err::Failure(nom::error::Error::new(self.bytes, nom::error::ErrorKind::OneOf)))
                }
            }
        }
    }
}

make_chunks! {
    SceneInfo1 = 3,
    SceneInfo2 = 12,
    SceneInfo3 = 20,

    Material = 4,
    //MaterialsXml = 11,

    Node = 0,
    Geometry = 5,
    Light = 10,
    //Camera = 19,
    
    KeyEvents = 21
    
    //PositionController = 1,
    //RotationController = 2,
    //LookatController = 6,
    //ColorController = 7,
    //AttenuationController = 8,
    //MultiplierController = 9,
    //HotspotController = 13,
    //FalloffController = 14,
    //FovController = 15,
    //FarClipController = 16,
    //NearClipController = 17,
    //TargetDistanceController = 18,
    //IkChainController = 22,
    //IkChainTargetController = 23,
    //CompositePositionController = 24,
    //CompositeRotationController = 25
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
pub struct SceneInfo1 {
    start_time: f64,
    end_time: f64,
}

#[derive(Debug, Parse)]
pub struct SceneInfo2 {
    start_time: f64,
    end_time: f64,

    author_tag: String,
    source_filename: String,
}

#[derive(Debug, Parse)]
pub struct SceneInfo3 {
    start_time: f64,
    end_time: f64,

    author_tag: String,
    source_filename: String,
    scene_type: String,
}

#[derive(Debug, Parse)]
pub struct Material {
    id: u32,
    name: String,
    parent_id: u32
}

#[derive(Debug, Parse)]
pub struct Node {
    id: u32,
    name: String,

    transform: vek::Mat4<f64>,
    pivot_transform: vek::Mat4<f64>,

    parent_id: u32
}

#[derive(Debug, Parse)]
pub struct Geometry {
    node_id: u32,

    /// ID of mesh material
    /// 0xFFFFFFFF == none
    material_id: u32,
    unknown1: u16,
    channels: Vec<GeometryChannel>,
    faces: Vec<GeometryFace>
}

#[derive(Debug)]
pub enum GeometryChannel {
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
pub struct GeometryFace {
    material_id: u32,
    unknown1: u32,

    loops: Vec<GeometryFaceloop>
}

#[derive(Debug, Parse)]
pub struct GeometryFaceloop {
    channel: u32,
    a: u32,
    b: u32,
    c: u32
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumTryFrom, Parse)]
#[repr(u32)]
pub enum LightType {
    Spot = 0,
    Directional = 1,
    Omni = 2
}

#[derive(Debug, PartialEq, Clone, Copy, EnumTryFrom, Parse)]
#[repr(u32)]
pub enum SpotlightShape {
    Rectangular = 0,
    Circular = 1
}

#[derive(Debug, Parse)]
pub struct Light {
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
#[derive(Debug, Parse)]
pub struct KeyEvents {
    events: Vec<KeyEvent>
}

#[derive(Parse,Debug)]
pub struct KeyEvent {
    id: u32,
    name: String,
    timestamp: f64,
    node_id: u32,    // The maya2017 exporter always writes 0xFFFFFFFF,
    event_type: String, // Exporter always writes "beat" or "trigger" here
    parameter_count: u32     // Exporter always writes 0
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
        print!("{:6} {:6} ", sec.offset, sec.length);
        match sec.try_into_chunk() {
            Ok(chunk) => println!("{:?}", chunk),
            Err(e) => println!("{:4} {:?} {:}", sec.type_code, e, AsHex(sec.bytes))
        }
    }
}