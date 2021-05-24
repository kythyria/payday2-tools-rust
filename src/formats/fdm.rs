//! Final Diesel Model format used in release versions of the game.

use std::convert::TryInto;

use nom::IResult;
use nom::combinator::map;
use nom::multi::{length_data, count};
use nom::number::complete::le_u32;
use nom::sequence::tuple;
use vek::{Vec2, Vec3, Vec4};

use crate::hashindex::Hash as Idstring;
use crate::util::parse_helpers;
use crate::util::parse_helpers::Parse;
use pd2tools_macros::{EnumTryFrom, Parse};

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Mat4f = vek::Mat4<f32>;
type Rgba = vek::Rgba<f32>;

pub struct UnparsedSection<'a> {
    pub r#type: u32,
    pub id: u32,
    pub data: &'a [u8]
}
impl<'a> UnparsedSection<'a> {
    fn parse(input: &'a [u8]) -> IResult<&'a[u8], UnparsedSection> {
        let (input, (r#type, id)) = tuple((le_u32, le_u32))(input)?;
        let (input, data) = length_data(le_u32)(input)?;
        Ok((input, UnparsedSection {
            r#type, id, data
        }))
    }
}

/// Header of a FDM file
///
/// No serialize because the logic to do that is surprisingly hard.
struct Header {
    length: u32,
    section_count: u32,
}
impl Header {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Header> {
        let (mut remain, (mut section_count, length)) = tuple((le_u32, le_u32))(input)?;

        if section_count == 0xFFFFFFFF {
            let (remain_1, count_1) = le_u32(remain)?;
            remain = remain_1;
            section_count = count_1;
        }

        Ok((remain, Header {
            section_count,
            length
        }))
    }
}

pub fn split_to_sections<'a>(input: &'a [u8]) -> IResult<&'a[u8], Vec<UnparsedSection>> {
    let (input, header) = Header::parse(input)?;
    count(UnparsedSection::parse, header.section_count as usize)(input)
}

#[derive(Copy, Clone, Eq, PartialEq, EnumTryFrom, Parse)]
pub enum SectionType {
    Object3D = 0x0ffcd100,
    LightSet = 0x33552583,
    Model = 0x62212d88,
    AuthorTag = 0x7623c465,
    Geometry = 0x7ab072d3,
    SimpleTexture = 0x072b4d37,
    CubicTexture = 0x2c5d6201,
    VolumetricTexture = 0x1d0b1808,
    Material = 0x3c54609c,
    MaterialGroup = 0x29276b1d,
    NormalManagingGP = 0x2c1f096f,
    TextureSpaceGP = 0x5ed2532f,
    PassThroughGP = 0xe3a3b1ca,
    SkinBones = 0x65cc1825,
    Topology = 0x4c507a13,
    TopologyIP = 0x03b634bd,
    Camera = 0x46bf31a7,
    Light = 0xffa13b80,
    ConstFloatController = 0x2060697e,
    StepFloatController = 0x6da951b2,
    LinearFloatController = 0x76bf5b66,
    BezierFloatController = 0x29743550,
    ConstVector3Controller = 0x5b0168d0,
    StepVector3Controller = 0x544e238f,
    LinearVector3Controller = 0x26a5128c,
    BezierVector3Controller = 0x28db639a,
    XYZVector3Controller = 0x33da0fc4,
    ConstRotationController = 0x2e540f3c,
    EulerRotationController = 0x033606e8,
    QuatStepRotationController = 0x007fb371,
    QuatLinearRotationController = 0x648a206c,
    QuatBezRotationController = 0x197345a5,
    LookAtRotationController = 0x22126dc0,
    LookAtConstrRotationController = 0x679d695b,
    IKChainTarget = 0x3d756e0c,
    IKChainRotationController = 0xf6c1eef7,
    CompositeVector3Controller = 0xdd41d329,
    CompositeRotationController = 0x95bb08f7,
    AnimationData = 0x5dc011b8,
    Animatable = 0x74f7363f,
    KeyEvents = 0x186a8bbf,
    D3DShader = 0x7f3552d1,
    D3DShaderPass = 0x214b1aaf,
    D3DShaderLibrary = 0x12812c1a,
}

/// Metadata about the model file. Release Diesel never, AFAIK, actually cares about this.
#[derive(Debug, Parse)]
pub struct AuthorSection {
    /// Very likely the "scene type" field
    name: Idstring,

    /// Email address of the author. In Overkill/LGL's tools, settable in the exporter settings.
    author_email: String,

    /// Absolute path of the original file.
    source_filename: String,
    unknown_2: u32
}

/// Scene object node
///
/// Blender calls this an Object, GLTF calls it a Node. Object3d on its own is an Empty node, just marking a point in
/// space, a joint, or suchlike. It may also occur as the start of a lamp, bounds, model, or camera
#[derive(Debug, Parse)]
pub struct Object3dSection {
    name: Idstring,
    animation_controllers: Vec<u32>,
    transform: Mat4f,
    parent: u32
}

/// Bounding box part of a Model
///
/// This can occur as the entirety of a Model if the `flavour` field is set
/// to 6. Such are used for collision volumes, where only the size is needed
/// and the physics engine can fill the rest in itself.
///
/// As part of `MeshModel`, it is used to control culling: if the bounding sphere
/// in particular is offscreen, the model will be culled.
#[derive(Debug, Parse)]
pub struct Bounds {
    /// One corner of the bounding box
    min: Vec3f,

    /// Another corner of the bounding box
    max: Vec3f,

    /// Radius of the bounding sphere whose centre is the model-space origin
    radius: f32,
    unknown_13: u32
}

#[derive(Debug, Parse)]
pub struct MeshModel {
    geometry_provider: u32,
    topology_ip: u32,
    render_atoms: Vec<RenderAtom>,
    material_group: u32,
    lightset: u32,
    bounds: Bounds,

    /// This seems to be flags? 1=shadowcaster, 2=has_opacity
    properties: u32,

    skinbones: u32
}

/// A single draw's worth of geometry
///
/// If you get this wrong Diesel doesn't usually crash but will display nonsense.
#[derive(Debug, Parse)]
pub struct RenderAtom {
    /// Starting position in the Geometry (vertex buffer)
    base_vertex: u32,

    /// Number of triangles to draw
    triangle_count: u32,

    /// Starting position in the Topology (index buffer)
    base_index: u32,

    /// Number of vertices in this RenderAtom
    geometry_slice_length: u32,

    /// Index of the material slot this uses.
    material: u32
}

/// Indirection to vertex and index data
///
/// It's unclear what the exact role is: Diesel itself has two more "Geometry Provider" classes that aren't used in any
/// file shipping with release Payday 2.
#[derive(Debug, Parse)]
pub struct PassthroughGPSection {
    geometry: u32,
    topology: u32
}

/// Indirection to index data
///
/// It's unclear what the role of this is at all, there are no other *IP classes that I can see.
#[derive(Debug, Parse)]
pub struct TopologyIPSection {
    topology: u32
}

/// Index buffer
#[derive(Debug, Parse)]
pub struct TopologySection {
    unknown_1: u32,
    faces: Vec<(u16, u16, u16)>,
    unknown_2: Vec<u8>,
    name: Idstring
}

/// Vertex attributes
///
/// I couldn't think of a definitely better way to do this, so a non-present attribute is represented by being empty.
/// It's what the previous incarnations of the model tool do.
///
/// The vertex attributes are almost in an order in the models in PD2 release. There's insufficient data to determine
/// exactly what it is and in any case there's two. I'm using the more common one, which is position, uv, 
#[derive(Default, Debug)]
pub struct GeometrySection {
    name: Idstring,

    position: Vec<Vec3f>,
    position_1: Vec<Vec3f>,
    normal_1: Vec<Vec3f>,
    color_0: Vec<Rgba>,
    color_1: Vec<Rgba>,
    tex_coord_0: Vec<Vec2f>,
    tex_coord_1: Vec<Vec2f>,
    tex_coord_2: Vec<Vec2f>,
    tex_coord_3: Vec<Vec2f>,
    tex_coord_4: Vec<Vec2f>,
    tex_coord_5: Vec<Vec2f>,
    tex_coord_6: Vec<Vec2f>,
    tex_coord_7: Vec<Vec2f>,
    weightcount_0: u32,
    blend_indices_0: Vec<Vec4<u8>>,
    blend_weight_0: Vec<Vec4f>,
    weightcount_1: u32,
    blend_indices_1: Vec<Vec4<u8>>,
    blend_weight_1: Vec<Vec4f>,
    normal: Vec<Vec3f>,
    binormal: Vec<Vec3f>,
    tangent: Vec<Vec3f>,

    // Just guessing here
    point_size: Vec<f32>
}
impl parse_helpers::Parse for GeometrySection {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (input, vertex_count) = u32::parse(input)?;
        let (input, descriptors) = Vec::<GeometryHeader>::parse(input)?;
        
        let mut input = input;
        let mut result = GeometrySection::default();

        macro_rules! match_attribute_parsers {
            ($src:expr, $vc:ident : $($ty:ident { $parser:expr => $place:expr } ),+) => {
                match $src {
                    $($ty => {
                        let (i, r) = count($parser, vertex_count as usize)(input)?;
                        input = i; $place = r;
                    }),+
                }
            }
        }

        for desc in descriptors {
            use GeometryAttributeType::*;
            
            fn parse_expand<'a, TItem, TAs>(input: &'a [u8]) -> IResult<&'a [u8], Vec4::<TItem>>
            where TAs: Parse, Vec4<TItem>: From<TAs> {
                map(TAs::parse, Vec4::from)(input)
            }

            match desc.attribute_type {
                BlendIndices0 | BlendWeight0 => { result.weightcount_0 = desc.attribute_size },
                BlendIndices1 | BlendWeight1 => { result.weightcount_1 = desc.attribute_size },
                _ => {}
            }

            let (idxparse, weightparse): (fn(&'a [u8])->IResult<&'a [u8], Vec4::<u8>>, fn(&'a [u8])->IResult<&'a [u8], Vec4::<f32>>) = match desc.attribute_size {
                //2 => ( parse_2to4::<u8>, parse_2to4::<f32> ),
                2 => ( parse_expand::<u8, Vec2<u8>>, parse_expand::<f32, Vec2<f32>> ),
                3 => ( parse_expand::<u8, Vec3<u8>>, parse_expand::<f32, Vec3<f32>> ),
                4 => ( parse_expand::<u8, Vec4<u8>>, parse_expand::<f32, Vec4<f32>> ),
                _ => unimplemented!("Unimplemented error handler")
            };

            match_attribute_parsers!{
                desc.attribute_type, vertex_count:

                Position { Vec3f::parse => result.position },
                Normal { Vec3f::parse => result.normal },
                Position1 { Vec3f::parse => result.position_1 },
                Normal1 { Vec3f::parse => result.normal_1 },
                Color0 { map(Rgba::parse, Rgba::shuffled_bgra) => result.color_0 },
                Color1 { map(Rgba::parse, Rgba::shuffled_bgra) => result.color_1 },
                TexCoord0 { Vec2f::parse => result.tex_coord_0 },
                TexCoord1 { Vec2f::parse => result.tex_coord_1 },
                TexCoord2 { Vec2f::parse => result.tex_coord_2 },
                TexCoord3 { Vec2f::parse => result.tex_coord_3 },
                TexCoord4 { Vec2f::parse => result.tex_coord_4 },
                TexCoord5 { Vec2f::parse => result.tex_coord_5 },
                TexCoord6 { Vec2f::parse => result.tex_coord_6 },
                TexCoord7 { Vec2f::parse => result.tex_coord_7 },
                Binormal { Vec3f::parse => result.binormal },
                Tangent { Vec3f::parse => result.tangent },
                BlendIndices0 { idxparse => result.blend_indices_0 },
                BlendIndices1 { idxparse => result.blend_indices_1 },
                BlendWeight0 { weightparse => result.blend_weight_0 },
                BlendWeight1 { weightparse => result.blend_weight_1 },
                PointSize { f32::parse => result.point_size }
            };
        }

        let (input, name) = Idstring::parse(input)?;
        result.name = name;
        Ok((input, result))
    }

    fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
        todo!();
        let vcount: u32 = self.position.len().try_into().unwrap();
        vcount.serialize(output)?;
    }
}

#[derive(Debug, Parse)]
pub struct GeometryHeader {
    pub attribute_size: u32,
    pub attribute_type: GeometryAttributeType
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, EnumTryFrom, Parse)]
pub enum GeometryAttributeType {
    Position = 1,
    Normal = 2,
    Position1 = 3,
    Normal1 = 4,
    Color0 = 5,
    Color1 = 6,
    TexCoord0 = 7,
    TexCoord1 = 8,
    TexCoord2 = 9,
    TexCoord3 = 10,
    TexCoord4 = 11,
    TexCoord5 = 12,
    TexCoord6 = 13,
    TexCoord7 = 14,
    BlendIndices0 = 15,
    BlendIndices1 = 16,
    BlendWeight0 = 17,
    BlendWeight1 = 18,
    PointSize = 19,
    Binormal = 20,
    Tangent = 21,
}

#[derive(EnumTryFrom)]
pub enum BlendComponentCount {
    Two = 2,
    Three = 3,
    Four = 4
}
impl Default for BlendComponentCount {
    fn default() -> Self {
        BlendComponentCount::Two
    }
}