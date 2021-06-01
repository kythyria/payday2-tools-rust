//! Final Diesel Model format used in release versions of the game.

use std::{convert::TryInto, marker::PhantomData};
use std::collections::HashMap;

use nom::IResult;
use nom::combinator::{all_consuming, map};
use nom::multi::{length_data, length_count, count};
use nom::number::complete::{le_u32, le_u64};
use nom::sequence::{tuple, terminated};
use vek::{Mat4, Vec2, Vec3, Vec4};

use crate::hashindex::Hash as Idstring;
use crate::util::AsHex;
use crate::util::parse_helpers;
use crate::util::parse_helpers::{ Parse, WireFormat };
use pd2tools_macros::{EnumTryFrom, Parse};

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Mat4f = vek::Mat4<f32>;
type Rgba = vek::Rgba<u8>;

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

macro_rules! make_document {
    (@vartype Unknown) => { Box<[u8]> };
    (@vartype $typename:ty) => { Box<$typename> };

    (@parse_arm $sec:ident, $tag:literal, $variantname:expr, Unknown) => { 
        Ok((b"", $variantname(Box::from($sec.data))))
    };
    (@parse_arm $sec:ident, $tag:literal, $variantname:expr, $typename:ident) => {
        {
            let ac = all_consuming($typename::parse);
            let boxed = map(ac, Box::from);
            map(boxed, $variantname)($sec.data)
        }
    };

    (@debug_arm $data:expr, $f:expr, Unknown, $vn:ident) => {
        write!($f, "{} {}", stringify!($vn), AsHex(&$data))
    };
    (@debug_arm $data:expr, $f:expr, $typ:ident, $vn:ident) => {
        <$typ as std::fmt::Debug>::fmt($data, $f)
    };

    ($( ($tag:literal, $variantname:ident, $typename:ident) )+) => {
        #[derive(Copy, Clone, Eq, PartialEq, EnumTryFrom, Parse, Debug)]
        pub enum SectionType {
            $(
                $variantname = $tag,
            )+
        }

        pub enum Section {
            $(
                $variantname(make_document!(@vartype $typename)),
            )+
        }

        impl std::fmt::Debug for Section {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match self {
                    $( Section::$variantname(d) => make_document!(@debug_arm d, f, $typename, $variantname), )+
                }
            }
        }

        pub fn parse_section<'a>(sec: &UnparsedSection<'a>) -> IResult<&'a [u8], Section> {
            match sec.r#type {
                $( $tag => make_document!(@parse_arm sec, $tag, Section::$variantname, $typename ), )+
                t => panic!("Wildly unknown section type {}", t)
            }
        }
    }
}

make_document! {
    (0x0ffcd100, Object3D,                       Object3dSection     )
    (0x33552583, LightSet,                       Unknown             )
    (0x62212d88, Model,                          ModelSection        )
    (0x7623c465, AuthorTag,                      AuthorSection       )
    (0x7ab072d3, Geometry,                       GeometrySection     )
    (0x072b4d37, SimpleTexture,                  Unknown             )
    (0x2c5d6201, CubicTexture,                   Unknown             )
    (0x1d0b1808, VolumetricTexture,              Unknown             )
    (0x3c54609c, Material,                       MaterialSection     )
    (0x29276b1d, MaterialGroup,                  MaterialGroupSection)
    (0x2c1f096f, NormalManagingGP,               Unknown             )
    (0x5ed2532f, TextureSpaceGP,                 Unknown             )
    (0xe3a3b1ca, PassthroughGP,                  PassthroughGPSection)
    (0x65cc1825, SkinBones,                      Unknown             )
    (0x4c507a13, Topology,                       TopologySection     )
    (0x03b634bd, TopologyIP,                     TopologyIPSection   )
    (0x46bf31a7, Camera,                         Unknown             )
    (0xffa13b80, Light,                          Unknown             )
    (0x2060697e, ConstFloatController,           Unknown             )
    (0x6da951b2, StepFloatController,            Unknown             )
    (0x76bf5b66, LinearFloatController,          Unknown             )
    (0x29743550, BezierFloatController,          Unknown             )
    (0x5b0168d0, ConstVector3Controller,         Unknown             )
    (0x544e238f, StepVector3Controller,          Unknown             )
    (0x26a5128c, LinearVector3Controller,        Unknown             )
    (0x28db639a, BezierVector3Controller,        Unknown             )
    (0x33da0fc4, XYZVector3Controller,           Unknown             )
    (0x2e540f3c, ConstRotationController,        Unknown             )
    (0x033606e8, EulerRotationController,        Unknown             )
    (0x007fb371, QuatStepRotationController,     Unknown             )
    (0x648a206c, QuatLinearRotationController,   Unknown             )
    (0x197345a5, QuatBezRotationController,      Unknown             )
    (0x22126dc0, LookAtRotationController,       Unknown             )
    (0x679d695b, LookAtConstrRotationController, Unknown             )
    (0x3d756e0c, IKChainTarget,                  Unknown             )
    (0xf6c1eef7, IKChainRotationController,      Unknown             )
    (0xdd41d329, CompositeVector3Controller,     Unknown             )
    (0x95bb08f7, CompositeRotationController,    Unknown             )
    (0x5dc011b8, AnimationData,                  Unknown             )
    (0x74f7363f, Animatable,                     Unknown             )
    (0x186a8bbf, KeyEvents,                      Unknown             )
    (0x7f3552d1, D3DShader,                      Unknown             )
    (0x214b1aaf, D3DShaderPass,                  Unknown             )
    (0x12812c1a, D3DShaderLibrary,               Unknown             )
}

pub fn parse_file<'a>(bytes: &'a [u8]) -> IResult<&'a [u8], HashMap<u32, Section>> {
    let (_, sections) = split_to_sections(bytes)?;
    let mut result = HashMap::<u32, Section>::new();
    for ups in sections {
        let (_, parsed) = parse_section(&ups)?;
        result.insert(ups.id, parsed);
    }
    return Ok((b"", result));
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
    pub name: Idstring,

    #[parse_as(AnimationControllerList)]
    pub animation_controllers: Vec<u32>,
    
    #[parse_as(Mat4WithPos<f32>)]
    pub transform: Mat4f,

    pub parent: u32
}

struct AnimationControllerList;
impl WireFormat<Vec<u32>> for AnimationControllerList {
    fn parse_into<'a>(input: &'a [u8]) -> IResult<&'a [u8], Vec<u32>> {
        let item = terminated(le_u32, le_u64);
        length_count(le_u32, item)(input)
    }

    fn serialize_from<O: std::io::Write>(data: &Vec<u32>, output: &mut O) -> std::io::Result<()> {
        let count: u32 = data.len().try_into().map_err(|_| std::io::ErrorKind::InvalidInput)?;
        count.serialize(output)?;
        for i in data.iter() {
            i.serialize(output)?;
            0u64.serialize(output)?;
        }
        Ok(())
    }
}

struct Mat4WithPos<T>{ _d: PhantomData<T> }
impl<T: Parse + Default> WireFormat<Mat4<T>> for Mat4WithPos<T> {
    fn parse_into<'a>(input: &'a [u8]) -> IResult<&'a [u8], Mat4<T>> {
        let (input, mut mat) = <vek::Mat4<T> as Parse>::parse(input)?;
        let (input, pos) = <vek::Vec3<T> as Parse>::parse(input)?;
        mat[(0,3)] = pos.x;
        mat[(1,3)] = pos.y;
        mat[(2,3)] = pos.z;
        Ok((input, mat))
    }

    fn serialize_from<O: std::io::Write>(data: &Mat4<T>, output: &mut O) -> std::io::Result<()> {
        data.serialize(output)?;
        data[(0,3)].serialize(output)?;
        data[(1,3)].serialize(output)?;
        data[(2,3)].serialize(output)
    }
}

#[derive(Debug, Parse)]
pub struct ModelSection {
    pub object: Object3dSection,
    pub data: ModelData
}

#[derive(Debug)]
pub enum ModelData {
    BoundsOnly(Bounds),
    Mesh(MeshModel)
}
impl Parse for ModelData {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (input, version) = u32::parse(input)?;
        match version {
            6 => map(Bounds::parse, ModelData::BoundsOnly)(input),
            _ => map(MeshModel::parse, ModelData::Mesh)(input)
        }
    }

    fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
        match self {
            ModelData::BoundsOnly(b) => {
                (6 as u32).serialize(output)?;
                b.serialize(output)
            },
            ModelData::Mesh(m) => {
                (3 as u32).serialize(output)?;
                m.serialize(output)
            }
        }
    }
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
    pub min: Vec3f,

    /// Another corner of the bounding box
    pub max: Vec3f,

    /// Radius of the bounding sphere whose centre is the model-space origin
    pub radius: f32,
    pub unknown_13: u32
}

#[derive(Debug, Parse)]
pub struct MeshModel {
    pub geometry_provider: u32,
    pub topology_ip: u32,
    pub render_atoms: Vec<RenderAtom>,
    pub material_group: u32,
    pub lightset: u32,
    pub bounds: Bounds,

    /// This seems to be flags? 1=shadowcaster, 2=has_opacity
    pub properties: u32,

    pub skinbones: u32
}

/// A single draw's worth of geometry
///
/// If you get this wrong Diesel doesn't usually crash but will display nonsense.
#[derive(Debug, Parse)]
pub struct RenderAtom {
    /// Starting position in the Geometry (vertex buffer). AFAICT this merely defines a slice, it doesn't get added to the indices.
    pub base_vertex: u32,

    /// Number of triangles to draw
    pub triangle_count: u32,

    /// Starting position in the Topology (index buffer), in indices, not triangles.
    pub base_index: u32,

    /// Number of vertices in this RenderAtom
    pub geometry_slice_length: u32,

    /// Index of the material slot this uses.
    pub material: u32
}

/// Indirection to vertex and index data
///
/// It's unclear what the exact role is: Diesel itself has two more "Geometry Provider" classes that aren't used in any
/// file shipping with release Payday 2.
#[derive(Debug, Parse)]
pub struct PassthroughGPSection {
    pub geometry: u32,
    pub topology: u32
}

/// Indirection to index data
///
/// It's unclear what the role of this is at all, there are no other *IP classes that I can see.
#[derive(Debug, Parse)]
pub struct TopologyIPSection {
    pub topology: u32
}

/// Index buffer
#[derive(Debug, Parse)]
pub struct TopologySection {
    pub unknown_1: u32,
    
    #[parse_as(VecOf3Tuple<u16>)]
    pub faces: Vec<(u16, u16, u16)>,

    pub unknown_2: Vec<u8>,
    pub name: Idstring
}

struct VecOf3Tuple<T>{ _d: PhantomData<T> }
impl<T: Parse> WireFormat<Vec<(T, T, T)>> for VecOf3Tuple<T> {
    fn parse_into<'a>(input: &'a [u8]) -> IResult<&'a [u8], Vec<(T, T, T)>> {
        length_count(map(le_u32, |i| i/3), <(T,T,T) as Parse>::parse)(input)
    }

    fn serialize_from<O: std::io::Write>(data: &Vec<(T, T, T)>, output: &mut O) -> std::io::Result<()> {
        let count_u = data.len()*3;
        let count: u32 = count_u.try_into().map_err(|_| std::io::ErrorKind::InvalidInput)?;
        count.serialize(output)?;
        for i in data.iter() {
            i.serialize(output)?;
        }
        Ok(())
    }
}

/// Vertex attributes
///
/// I couldn't think of a definitely better way to do this, so a non-present attribute is represented by being empty.
/// It's what the previous incarnations of the model tool do.
///
/// The vertex attributes are almost in an order in the models in PD2 release. There's insufficient data to determine
/// exactly what it is and in any case there's two. I'm using the more common one. 
#[derive(Default, Debug)]
pub struct GeometrySection {
    pub name: Idstring,

    pub position: Vec<Vec3f>,
    pub position_1: Vec<Vec3f>,
    pub normal_1: Vec<Vec3f>,
    pub color_0: Vec<Rgba>,
    pub color_1: Vec<Rgba>,
    pub tex_coord_0: Vec<Vec2f>,
    pub tex_coord_1: Vec<Vec2f>,
    pub tex_coord_2: Vec<Vec2f>,
    pub tex_coord_3: Vec<Vec2f>,
    pub tex_coord_4: Vec<Vec2f>,
    pub tex_coord_5: Vec<Vec2f>,
    pub tex_coord_6: Vec<Vec2f>,
    pub tex_coord_7: Vec<Vec2f>,
    pub weightcount_0: u32,
    pub blend_indices_0: Vec<Vec4<u16>>,
    pub blend_weight_0: Vec<Vec4f>,
    pub weightcount_1: u32,
    pub blend_indices_1: Vec<Vec4<u16>>,
    pub blend_weight_1: Vec<Vec4f>,
    pub normal: Vec<Vec3f>,
    pub binormal: Vec<Vec3f>,
    pub tangent: Vec<Vec3f>,

    // Just guessing here
    pub point_size: Vec<f32>
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
            
            fn expand<'a, TItem, TAs>(input: &'a [u8]) -> IResult<&'a [u8], Vec4::<TItem>>
            where TAs: Parse, Vec4<TItem>: From<TAs> {
                map(TAs::parse, Vec4::from)(input)
            }

            match desc.attribute_type {
                BlendWeight0 => { result.weightcount_0 = desc.attribute_format },
                BlendWeight1 => { result.weightcount_1 = desc.attribute_format },
                _ => {}
            }

            let (idxparse, weightparse): (fn(&'a [u8])->IResult<&'a [u8], Vec4::<u16>>, fn(&'a [u8])->IResult<&'a [u8], Vec4::<f32>>) = match desc.attribute_format {
                2 =>( expand::<u16, Vec2<u16>>, expand::<f32, Vec2<f32>> ),
                3 => ( expand::<u16, Vec3<u16>>, expand::<f32, Vec3<f32>> ),
                4 => ( expand::<u16, Vec4<u16>>, expand::<f32, Vec4<f32>> ),
                // Really this should be an error but we need a default for the non-weight-related ones anyway.
                _ => ( expand::<u16, Vec4<u16>>, expand::<f32, Vec4<f32>> )
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
        let vcount: u32 = self.position.len().try_into().unwrap();
        vcount.serialize(output)?;

        let mut headers = Vec::<GeometryHeader>::with_capacity(21);

        macro_rules! attribute_writers {
            (@item_header $attrib:ident, $format:expr, $typ:ident) => {
                if self.$attrib.len() > 0 { headers.push(GeometryHeader { attribute_format: $format, attribute_type: GeometryAttributeType::$typ }) }
            };
            (@item_writer $attrib:ident, $writer:expr) => { if self.$attrib.len() > 0 { $writer(output, &self.$attrib)?; } };
            (@item_writer $attrib:ident) => { if self.$attrib.len() > 0 { self.$attrib.serialize(output)?; } };
            ($( $attrib:ident ($format:expr, $typ:ident $(, $writer:expr)? ); )+) => {
                $( attribute_writers!(@item_header $attrib, $format, $typ); )+
                $( attribute_writers!(@item_writer $attrib $(, $writer)? ); )+
            };
        }

        fn serialize_iter<I: Parse, D: Iterator<Item=I>, O: std::io::Write>(output: &mut O, data: &mut D) -> std::io::Result<()> {
            for i in data {
                i.serialize(output)?;
            }
            Ok(())
        }
        fn write_4as2<O: std::io::Write>(output: &mut O, data: &Vec<Vec4f>) -> std::io::Result<()> {
            serialize_iter(output, &mut data.iter().map(|i| Vec2f{x:i.x, y:i.y}))
        }
        fn write_4as3<O: std::io::Write>(output: &mut O, data: &Vec<Vec4f>) -> std::io::Result<()> {
            serialize_iter(output, &mut data.iter().map(|i| Vec3f{x:i.x, y:i.y, z:i.z}))
        }
        fn write_4as4<O: std::io::Write>(output: &mut O, data: &Vec<Vec4f>) -> std::io::Result<()> {
            serialize_iter(output, &mut data.iter().map(Vec4f::clone))
        }
        let weight_0_write : fn(&mut O, &Vec<Vec4f>) -> std::io::Result<()> = match self.weightcount_0 {
            2 => write_4as2,
            3 => write_4as3,
            4 => write_4as4,
            _ => todo!("Sensibly handle needing the wrong number of weights")
        };
        let weight_1_write : fn(&mut O, &Vec<Vec4f>) -> std::io::Result<()> = match self.weightcount_1 {
            2 => write_4as2,
            3 => write_4as3,
            4 => write_4as4,
            _ => todo!("Sensibly handle needing the wrong number of weights")
        };

        attribute_writers!{
            position(3, Position);
            tex_coord_0(2, TexCoord0);
            tex_coord_1(2, TexCoord1);
            tex_coord_2(2, TexCoord2);
            tex_coord_3(2, TexCoord3);
            tex_coord_4(2, TexCoord4);
            tex_coord_5(2, TexCoord5);
            tex_coord_6(2, TexCoord6);
            tex_coord_7(2, TexCoord7);
            color_0(5, Color0);
            blend_indices_0(7, BlendIndices0);
            blend_weight_0(self.weightcount_0, BlendWeight0, weight_0_write);
            blend_indices_1(7, BlendIndices1);
            blend_weight_1(self.weightcount_1, BlendWeight1, weight_1_write);
            normal(3, Normal);
            binormal(3, Binormal);
            tangent(3, Tangent);
            position_1(3, Position1);
            color_1(5, Color1);
            normal_1(3, Normal1);
            point_size(1, PointSize);
        }

        self.name.serialize(output)
    }
}

#[derive(Debug, Parse)]
pub struct GeometryHeader {
    pub attribute_format: u32,
    pub attribute_type: GeometryAttributeType
}
impl GeometryHeader {
    fn component_count(&self) -> u32 {
        use GeometryAttributeType::*;
        match self.attribute_type {
            Position | Normal | Position1 | Normal1 | Color0 | Color1 | Binormal | Tangent => 3,
            TexCoord0 | TexCoord1 | TexCoord2 | TexCoord3 | TexCoord4 | TexCoord5 | TexCoord6 | TexCoord7 => 2,
            PointSize => 1,
            BlendIndices0 | BlendIndices1 => 4,
            BlendWeight0 | BlendWeight1 => self.attribute_format
        }
    }

    fn byte_count(&self) -> u32 {
        let counts = [0, 4, 8, 12, 16, 4, 4, 8, 12];
        counts[self.attribute_format as usize]
    }
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

#[derive(Debug, Parse)]
pub struct MaterialGroupSection {
    pub material_ids: Vec<u32>
}

#[derive(Debug, Parse)]
pub struct MaterialSection {
    pub name: u64,

    #[skip_before(48)]
    pub items: Vec<(u32, u32)>
}