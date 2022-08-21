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

use std::{path::Path};
use vek::{Rgb, Vec2, Vec3};

use crate::util::binaryreader::*;

use crate::util::AsHex;
use crate::util::read_helpers::{TryFromIndexedLE, TryFromBytesError};
use crate::util::parse_helpers;
use pd2tools_macros::{EnumTryFrom, ItemReader};

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
            fn try_into_chunk(&self) -> (&'a [u8], Result<Chunk, ReadError>) {

                let mut reader = self.bytes;
                let res = match self.type_code {
                    $($tag => {
                        reader.read_item_as::<$name>().map(Chunk::$name)
                    }),+
                    d => Err(ReadError::BadDiscriminant("ChunkId", d as u128))
                };
                (reader, res)
            }
        }
    }
}

make_chunks! {
    SceneInfo1 = 3,
    SceneInfo2 = 12,
    SceneInfo3 = 20,

    Material = 4,
    MaterialsXml = 11,

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

#[derive(Debug, ItemReader)]
pub struct SceneInfo1 {
    start_time: f64,
    end_time: f64,
}

#[derive(Debug, ItemReader)]
pub struct SceneInfo2 {
    start_time: f64,
    end_time: f64,

    author_tag: String,
    source_filename: String,
}

#[derive(Debug, ItemReader)]
pub struct SceneInfo3 {
    start_time: f64,
    end_time: f64,

    author_tag: String,
    source_filename: String,
    scene_type: String,
}

#[derive(Debug, ItemReader)]
pub struct Material {
    id: u32,
    name: String,
    parent_id: u32
}

#[derive(Debug, ItemReader)]
pub struct MaterialsXml {
    xml: String
}

#[derive(Debug, ItemReader)]
pub struct Node {
    id: u32,
    name: String,

    transform: vek::Mat4<f64>,
    pivot_transform: vek::Mat4<f64>,

    parent_id: u32
}

#[derive(Debug, ItemReader)]
pub struct Geometry {
    node_id: u32,

    /// ID of mesh material
    /// 0xFFFFFFFF == none
    material_id: u32,
    unknown1: u16,
    channels: Vec<GeometryChannel>,
    faces: Vec<GeometryFace>
}

pub struct GeometrySkin {
    root_node_id: u32,
    postmul_transform: vek::Mat4<f64>,
    weights: Vec<VertexWeight>,

    /// List of lists of bone IDs.
    bonesets: Vec<Vec<u32>>
}

#[derive(Debug, Clone, Copy, ItemReader)]
pub struct VertexWeight {
    bone_id: u32,
    weight: f64
}

#[derive(Debug, Clone, Copy, ItemReader)]
pub struct BoundingBox {
    min: Vec3<f64>,
    max: Vec3<f64>
}

#[derive(Debug, ItemReader)]
pub enum GeometryChannel {
    #[tag(0)] Position(u32, Vec<Vec3<f64>>),
    #[tag(1)] TexCoord(u32, Vec<Vec2<f64>>),
    #[tag(2)] Normal  (u32, Vec<Vec3<f64>>),
    #[tag(3)] Binormal(u32, Vec<Vec3<f64>>),
    #[tag(4)] Tangent (u32, Vec<Vec3<f64>>),
    #[tag(5)] Colour  (u32, Vec<Rgb<f64>>)
}

#[derive(Debug, ItemReader)]
pub struct GeometryFace {
    material_id: u32,
    unknown1: u32,

    loops: Vec<GeometryFaceloop>
}

#[derive(Debug, ItemReader)]
pub struct GeometryFaceloop {
    channel: u32,
    a: u32,
    b: u32,
    c: u32
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumTryFrom, ItemReader)]
#[repr(u32)]
pub enum LightType {
    Spot = 0,
    Directional = 1,
    Omni = 2
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, EnumTryFrom, ItemReader)]
#[repr(u32)]
pub enum SpotlightShape {
    Rectangular = 0,
    Circular = 1
}

#[derive(Debug, ItemReader)]
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
#[derive(Debug, ItemReader)]
pub struct KeyEvents {
    events: Vec<KeyEvent>
}

#[derive(Debug, ItemReader)]
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
        let (remain, res) = sec.try_into_chunk();
        match res {
            Ok(chunk) => println!("{:?} {:}", chunk, AsHex(remain)),
            Err(e) => println!("{:4} {:?} {:}", sec.type_code, e, sec.length - remain.len())
        }
    }
}