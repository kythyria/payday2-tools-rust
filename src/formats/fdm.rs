//! Final Diesel Model format used in release versions of the game.

pub mod container;
pub use container::*;

use std::convert::TryInto;

use vek::{Mat4, Vec2, Vec3, Vec4};
use thiserror::Error;

use crate::hashindex::Hash as Idstring;
use crate::util::AsHex;
use crate::util::binaryreader;
use crate::util::binaryreader::*;
use pd2tools_macros::{EnumTryFrom, ItemReader};

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Mat4f = vek::Mat4<f32>;
type Rgba = vek::Rgba<u8>;

#[derive(Clone, Debug, Error)]
pub enum ParseError {
    #[error("Somehow failed to parse file or section headers at offset {0}")]
    BadHeaders(usize),

    #[error("Section {id} whose data is at {data_offset} has unknown type {r#type:x}")]
    UnknownSectionType {
        id: u32,
        r#type: u32,
        data_offset: usize
    },

    #[error("Parse error in section {id} at offset {location}")]
    BadSection { id: u32, location: usize },

    #[error("Unexpected EOF during section {id}")]
    TruncatedSection { id: u32 },

    #[error("Unexpected EOF reading section {got} of {expected}")]
    NotEnoughSections { got: u32, expected: u32 },
}

macro_rules! make_document {
    (@vartype Unknown) => { Box<[u8]> };
    (@vartype $typename:ty) => { Box<$typename> };

    (@read_arm $variant:expr, Unknown, $data:expr) => {
        Ok($variant(Box::from($data)))
    };
    (@read_arm $variant:expr, $typename:ty, $data:expr) => {
        {
            Ok($variant(Box::new($data.read_item_as::<$typename>()?)))
        }
    };

    (@write_arm $stream:expr, $s:expr, Unknown) => {
        $stream.write_item($s)
    };
    (@write_arm $stream:expr, $s:expr, $typ:ty) => {
        $stream.write_item($s.as_ref())
    };

    (@debug_arm $data:expr, $f:expr, Unknown, $vn:ident) => {
        write!($f, "{} {}", stringify!($vn), AsHex(&$data))
    };
    (@debug_arm $data:expr, $f:expr, $typ:ident, $vn:ident) => {
        <$typ as std::fmt::Debug>::fmt($data, $f)
    };

    ($( ($tag:literal, $variantname:ident, $typename:ident) )+) => {
        #[derive(Copy, Clone, Eq, PartialEq, Debug, PartialOrd, Ord, EnumTryFrom, ItemReader)]
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
        impl Section {
            pub fn tag(&self) -> SectionType {
                match self {
                    $( Section::$variantname(_) => SectionType::$variantname ),+
                }
            }

            pub fn write_data(&self, stream: &mut impl WriteExt) -> Result<(), ReadError> {
                match self {
                    $( Section::$variantname(s) => make_document!(@write_arm stream, s, $typename), )+
                }
            }
        }

        impl std::fmt::Debug for Section {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match self {
                    $( Section::$variantname(d) => make_document!(@debug_arm d, f, $typename, $variantname), )+
                }
            }
        }

        pub fn read_section<'a>(sec_type: SectionType, mut data: &'a [u8]) -> Result<Section, ReadError> {
            match sec_type {
                $( SectionType::$variantname => make_document!(@read_arm Section::$variantname, $typename, data), )+
            }
        }
    }
}

make_document! {
    (0x0ffcd100, Object3D,                       Object3dSection                      )
    (0x33552583, LightSet,                       Unknown                              )
    (0x62212d88, Model,                          ModelSection                         )
    (0x7623c465, AuthorTag,                      AuthorSection                        )
    (0x7ab072d3, Geometry,                       GeometrySection                      )
    (0x072b4d37, SimpleTexture,                  Unknown                              )
    (0x2c5d6201, CubicTexture,                   Unknown                              )
    (0x1d0b1808, VolumetricTexture,              Unknown                              )
    (0x3c54609c, Material,                       MaterialSection                      )
    (0x29276b1d, MaterialGroup,                  MaterialGroupSection                 )
    (0x2c1f096f, NormalManagingGP,               Unknown                              )
    (0x5ed2532f, TextureSpaceGP,                 Unknown                              )
    (0xe3a3b1ca, PassthroughGP,                  PassthroughGPSection                 )
    (0x65cc1825, SkinBones,                      Unknown                              )
    (0x4c507a13, Topology,                       TopologySection                      )
    (0x03b634bd, TopologyIP,                     TopologyIPSection                    )
    (0x46bf31a7, Camera,                         Unknown                              )
    (0xffa13b80, Light,                          LightSection                         )
    (0x2060697e, ConstFloatController,           Unknown                              )
    (0x6da951b2, StepFloatController,            Unknown                              )
    (0x76bf5b66, LinearFloatController,          LinearFloatControllerSection         )
    (0x29743550, BezierFloatController,          Unknown                              )
    (0x5b0168d0, ConstVector3Controller,         Unknown                              )
    (0x544e238f, StepVector3Controller,          Unknown                              )
    (0x26a5128c, LinearVector3Controller,        LinearVector3ControllerSection       )
    (0x28db639a, BezierVector3Controller,        Unknown                              )
    (0x33da0fc4, XYZVector3Controller,           Unknown                              )
    (0x2e540f3c, ConstRotationController,        Unknown                              )
    (0x033606e8, EulerRotationController,        Unknown                              )
    (0x007fb371, QuatStepRotationController,     Unknown                              )
    (0x648a206c, QuatLinearRotationController,   QuatLinearRotationControllerSection  )
    (0x197345a5, QuatBezRotationController,      Unknown                              )
    (0x22126dc0, LookAtRotationController,       Unknown                              )
    (0x679d695b, LookAtConstrRotationController, LookAtConstrRotationControllerSection)
    (0x3d756e0c, IKChainTarget,                  Unknown                              )
    (0xf6c1eef7, IKChainRotationController,      Unknown                              )
    (0xdd41d329, CompositeVector3Controller,     Unknown                              )
    (0x95bb08f7, CompositeRotationController,    Unknown                              )
    (0x5dc011b8, AnimationData,                  AnimationDataSection                 )
    (0x74f7363f, Animatable,                     Unknown                              )
    (0x186a8bbf, KeyEvents,                      Unknown                              )
    (0x7f3552d1, D3DShader,                      Unknown                              )
    (0x214b1aaf, D3DShaderPass,                  Unknown                              )
    (0x12812c1a, D3DShaderLibrary,               Unknown                              )

    (0x7c7844fd, ModelToolHashes,                ModelToolHashSection                 )
}

pub fn parse_stream(input: &mut impl ReadExt) -> Result<DieselContainer, ReadError> {
    input.read_item()
}

/// Metadata about the model file. Release Diesel never, AFAIK, actually cares about this.
#[derive(Debug, ItemReader)]
pub struct AuthorSection {
    /// Very likely the "scene type" field
    name: Idstring,

    /// Email address of the author. In Overkill/LGL's tools, settable in the exporter settings.
    #[read_as(NullTerminatedString)]
    author_email: String,

    /// Absolute path of the original file.
    #[read_as(NullTerminatedString)]
    source_filename: String,
    unknown_2: u32
}

/// Scene object node
///
/// Blender calls this an Object, GLTF calls it a Node. Object3d on its own is an Empty node, just marking a point in
/// space, a joint, or suchlike. It may also occur as the start of a lamp, bounds, model, or camera
#[derive(Debug, ItemReader)]
pub struct Object3dSection {
    pub name: Idstring,

    #[read_as(AnimationControllerList)]
    pub animation_controllers: Vec<u32>,
    
    #[read_as(Mat4fWithPos)]
    pub transform: Mat4f,

    pub parent: u32
}

struct AnimationControllerList;
impl ItemReader for AnimationControllerList {
    type Error = ReadError;
    type Item = Vec<u32>;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let count: u32 = stream.read_item()?;
        let mut res = Vec::<u32>::with_capacity(count.try_into().unwrap());
        for _ in 0..count {
            res.push(stream.read_item()?);
            let _ = stream.read_item_as::<u64>()?;
        }
        Ok(res)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let wire_count: u32 = item.len()
            .try_into()
            .map_err(|_| ReadError::TooManyItems(item.len(), "u32", "u32"))?;
        stream.write_item(&wire_count)?;
        for i in item {
            stream.write_item(i)?;
            stream.write_item(&0u64)?;
        }
        Ok(())
    }
}

struct Mat4fWithPos;

impl ItemReader for Mat4fWithPos {
    type Error = ReadError;
    type Item = Mat4<f32>;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let mut mat: Mat4<f32> = stream.read_item()?;
        let pos: Vec3<f32> = stream.read_item()?;
        mat[(0,3)] = pos.x;
        mat[(1,3)] = pos.y;
        mat[(2,3)] = pos.z;
        Ok(mat)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        stream.write_item(item)?;
        stream.write_item(&item[(0,3)])?;
        stream.write_item(&item[(1,3)])?;
        stream.write_item(&item[(2,3)])
    }
}

#[derive(Debug, ItemReader)]
pub struct ModelSection {
    pub object: Object3dSection,
    pub data: ModelData
}

#[derive(Debug, ItemReader)]
pub enum ModelData {
    #[tag(6)] BoundsOnly(Bounds),
    #[tag(3)] Mesh(MeshModel)
}

/// Bounding box part of a Model
///
/// This can occur as the entirety of a Model if the `flavour` field is set
/// to 6. Such are used for collision volumes, where only the size is needed
/// and the physics engine can fill the rest in itself.
///
/// As part of `MeshModel`, it is used to control culling: if the bounding sphere
/// in particular is offscreen, the model will be culled.
#[derive(Debug, ItemReader)]
pub struct Bounds {
    /// One corner of the bounding box
    pub min: Vec3f,

    /// Another corner of the bounding box
    pub max: Vec3f,

    /// Radius of the bounding sphere whose centre is the model-space origin
    pub radius: f32,
    pub unknown_13: u32
}

#[derive(Debug, ItemReader)]
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
#[derive(Debug, ItemReader)]
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

/// Light source
#[derive(Debug, ItemReader)]
pub struct LightSection {
    pub object: Object3dSection,
    pub unknown_1: u8,
    pub light_type: LightType,
    pub color: vek::Rgba<f32>,
    pub near_range: f32,
    pub far_range: f32,
    pub unknown_6: f32,
    pub unknown_7: f32,
    pub unknown_8: f32
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, EnumTryFrom, ItemReader)]
pub enum LightType {
    Omnidirectional = 1,
    Spot = 2
}

/// Indirection to vertex and index data
///
/// It's unclear what the exact role is: Diesel itself has two more "Geometry Provider" classes that aren't used in any
/// file shipping with release Payday 2.
#[derive(Debug, ItemReader)]
pub struct PassthroughGPSection {
    pub geometry: u32,
    pub topology: u32
}

/// Indirection to index data
///
/// It's unclear what the role of this is at all, there are no other *IP classes that I can see.
#[derive(Debug, ItemReader)]
pub struct TopologyIPSection {
    pub topology: u32
}

/// Index buffer
#[derive(Debug, ItemReader)]
pub struct TopologySection {
    pub unknown_1: u32,
    
    pub faces: Vec<u16>,

    pub unknown_2: Vec<u8>,
    pub name: Idstring
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
impl ItemReader for GeometrySection {
    type Error = ReadError;
    type Item = GeometrySection;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let mut result = GeometrySection::default();

        let vertex_count: u32 = stream.read_item()?;
        let descriptors: Vec<GeometryHeader> = stream.read_item()?;

        fn read_vec<T: ItemReader>(stream: &mut impl ReadExt, count: u32) -> Result<Vec<T::Item>, T::Error> {
            let mut buf = Vec::with_capacity(count as usize);
            for _ in 0..count {
                buf.push(stream.read_item_as::<T>()?)
            }
            Ok(buf)
        }

        fn read_vec_as<T, V>(stream: &mut impl ReadExt, count: u32) -> Result<Vec<Vec4<T::Item>>, ReadError>
        where
            T: ItemReader<Error=ReadError>,
            V: ItemReader<Error=ReadError>,
            Vec4<T::Item>: From<V::Item>
        {
            let mut buf = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let item = stream.read_item_as::<V>()?;
                buf.push(item.into());
            }
            Ok(buf)
        }

        for desc in descriptors {
            use GeometryAttributeType::*;

            match desc.attribute_type {
                BlendWeight0 => { result.weightcount_0 = desc.attribute_format },
                BlendWeight1 => { result.weightcount_1 = desc.attribute_format },
                _ => {}
            }

            type BlendReader<T,S> = fn(&mut S, count: u32) -> Result<Vec<Vec4<T>>, ReadError>;
            let (idxread, weightread): (BlendReader<u16,R>, BlendReader<f32,R>) = match desc.attribute_format {
                2 => (read_vec_as::<u16, Vec2<u16>>, read_vec_as::<f32, Vec2<f32>>),
                3 => (read_vec_as::<u16, Vec3<u16>>, read_vec_as::<f32, Vec3<f32>>),
                4 => (read_vec_as::<u16, Vec4<u16>>, read_vec_as::<f32, Vec4<f32>>),
                _ => (read_vec_as::<u16, Vec4<u16>>, read_vec_as::<f32, Vec4<f32>>),
            };

            match desc.attribute_type {
                Position => result.position = read_vec::<Vec3f>(stream, vertex_count)?,
                Normal => result.normal = read_vec::<Vec3f>(stream, vertex_count)?,
                Position1 => result.position_1 = read_vec::<Vec3f>(stream, vertex_count)?,
                Normal1 => result.normal_1 = read_vec::<Vec3f>(stream, vertex_count)?,
                Color0 => result.color_0 = read_vec::<Bgra<u8>>(stream, vertex_count)?,
                Color1 => result.color_1 = read_vec::<Bgra<u8>>(stream, vertex_count)?,
                TexCoord0 => result.tex_coord_0 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord1 => result.tex_coord_1 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord2 => result.tex_coord_2 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord3 => result.tex_coord_3 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord4 => result.tex_coord_4 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord5 => result.tex_coord_5 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord6 => result.tex_coord_6 = read_vec::<Vec2f>(stream, vertex_count)?,
                TexCoord7 => result.tex_coord_7 = read_vec::<Vec2f>(stream, vertex_count)?,
                BlendIndices0 => result.blend_indices_0 = idxread(stream, vertex_count)?,
                BlendIndices1 => result.blend_indices_1 = idxread(stream, vertex_count)?,
                BlendWeight0 => result.blend_weight_0 = weightread(stream, vertex_count)?,
                BlendWeight1 => result.blend_weight_1 = weightread(stream, vertex_count)?,
                PointSize => result.point_size = read_vec::<f32>(stream, vertex_count)?,
                Binormal => result.binormal = read_vec::<Vec3f>(stream, vertex_count)?,
                Tangent => result.tangent = read_vec::<Vec3f>(stream, vertex_count)?,
            }
        }

        Ok(result)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let vcount: u32 = item.position.len().try_into().unwrap();
        stream.write_item(&vcount)?;

        let mut headers = Vec::<GeometryHeader>::with_capacity(21);

        fn write_attr<'a, T>(output: &mut impl WriteExt, data: impl Iterator<Item=&'a T::Item>) -> Result<(), T::Error>
        where
            T: ItemReader,
            T::Item: 'a
        {
            for i in data { output.write_item_as::<T>(i)? }
            Ok(())
        }

        fn write_vecvec<'a, TA,TW>(output: &mut impl WriteExt, data: impl Iterator<Item=&'a TA>)
        -> Result<(), ReadError>
        where
            TA: 'a + Clone,
            TW: ItemReader<Error=ReadError>,
            TW::Item: From<TA>
        {
            for i in data {
                let w = <TW::Item>::from(i.clone());
                output.write_item_as::<TW>(&w)?;
            }
            Ok(())
        }

        type BlendWriter<S,I> = fn(&mut S, I) -> Result<(), ReadError>;

        let (wr_idx_0, wr_wgt_0): (BlendWriter<_,_>, BlendWriter<_,_>) = match item.weightcount_0 {
            2 => (write_vecvec::<Vec4<u16>, Vec2<u16>>, write_vecvec::<Vec4f, Vec2f>),
            3 => (write_vecvec::<Vec4<u16>, Vec3<u16>>, write_vecvec::<Vec4f, Vec3f>),
            4 => (write_vecvec::<Vec4<u16>, Vec4<u16>>, write_vecvec::<Vec4f, Vec4f>),
            _ => todo!("Sensibly handle needing the wrong number of weights")
        };

        let (wr_idx_1, wr_wgt_1): (BlendWriter<_,_>, BlendWriter<_,_>) = match item.weightcount_1 {
            2 => (write_vecvec::<Vec4<u16>, Vec2<u16>>, write_vecvec::<Vec4f, Vec2f>),
            3 => (write_vecvec::<Vec4<u16>, Vec3<u16>>, write_vecvec::<Vec4f, Vec3f>),
            4 => (write_vecvec::<Vec4<u16>, Vec4<u16>>, write_vecvec::<Vec4f, Vec4f>),
            _ => todo!("Sensibly handle needing the wrong number of weights")
        };

        macro_rules! write_attributes {
            ($( $attrname:ident ($format:expr, $a_typ:ident, $write:expr ); )+) => {
                $(
                    if item.$attrname.len() > 0 {
                        headers.push(GeometryHeader{
                            attribute_format: $format,
                            attribute_type: GeometryAttributeType::$a_typ
                        });
                    }
                )+
                stream.write_item(&headers)?;
                $(
                    if item.$attrname.len() > 0 {
                        $write(stream, item.$attrname.iter())?;
                    }
                )+
            }
        }

        write_attributes!{
            position(3, Position, write_attr::<Vec3f>);
            tex_coord_0(2, TexCoord0, write_attr::<Vec2f>);
            tex_coord_1(2, TexCoord1, write_attr::<Vec2f>);
            tex_coord_2(2, TexCoord2, write_attr::<Vec2f>);
            tex_coord_3(2, TexCoord3, write_attr::<Vec2f>);
            tex_coord_4(2, TexCoord4, write_attr::<Vec2f>);
            tex_coord_5(2, TexCoord5, write_attr::<Vec2f>);
            tex_coord_6(2, TexCoord6, write_attr::<Vec2f>);
            tex_coord_7(2, TexCoord7, write_attr::<Vec2f>);
            color_0(5, Color0, write_attr::<Bgra<u8>>);

            blend_indices_0(item.weightcount_0, BlendIndices0, wr_idx_0);
            blend_weight_0(item.weightcount_0, BlendWeight0, wr_wgt_0);
            blend_indices_1(item.weightcount_1, BlendIndices1, wr_idx_1);
            blend_weight_1(item.weightcount_1, BlendWeight1, wr_wgt_1);

            normal(3, Normal, write_attr::<Vec3f>);
            binormal(3, Binormal, write_attr::<Vec3f>);
            tangent(3, Tangent, write_attr::<Vec3f>);
            position_1(3, Position1, write_attr::<Vec3f>);
            color_1(5, Color1, write_attr::<Bgra<u8>>);
            normal_1(3, Normal1, write_attr::<Vec3f>);
            point_size(1, PointSize, write_attr::<f32>);
        }

        stream.write_item(&item.name)
    }
}

#[derive(Debug, ItemReader)]
pub struct GeometryHeader {
    pub attribute_format: u32,
    pub attribute_type: GeometryAttributeType
}
impl GeometryHeader {
    pub fn component_count(&self) -> u32 {
        use GeometryAttributeType::*;
        match self.attribute_type {
            Position | Normal | Position1 | Normal1 | Color0 | Color1 | Binormal | Tangent => 3,
            TexCoord0 | TexCoord1 | TexCoord2 | TexCoord3 | TexCoord4 | TexCoord5 | TexCoord6 | TexCoord7 => 2,
            PointSize => 1,
            BlendIndices0 | BlendIndices1 => 4,
            BlendWeight0 | BlendWeight1 => self.attribute_format
        }
    }

    pub fn byte_count(&self) -> u32 {
        let counts = [0, 4, 8, 12, 16, 4, 4, 8, 12];
        counts[self.attribute_format as usize]
    }
}
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, EnumTryFrom, ItemReader)]
#[repr(u32)]
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

#[derive(EnumTryFrom, ItemReader)]
#[repr(u32)]
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

#[derive(Debug, ItemReader)]
pub struct MaterialGroupSection {
    pub material_ids: Vec<u32>
}

#[derive(Debug, ItemReader)]
pub struct MaterialSection {
    pub name: u64,

    #[skip_before(48)]
    pub items: Vec<(u32, u32)>
}

#[derive(Debug, ItemReader)]
pub struct AnimationDataSection {
    pub name: Idstring,
    pub unknown_2: u32,
    pub duration: f32,
    pub keyframes: Vec<f32>
}

#[derive(Debug, ItemReader)]
pub struct LinearVector3ControllerSection {
    pub name: Idstring,
    pub flags: u32,
    pub unknown_1: u32,
    pub duration: f32,
    pub keyframes: Vec<(f32, Vec3f)>
}

#[derive(Debug, ItemReader)]
pub struct LinearFloatControllerSection {
    pub name: Idstring,
    pub flags: u32,
    pub unknown_1: u32,
    pub duration: f32,
    pub keyframes: Vec<(f32, f32)>
}

#[derive(Debug, ItemReader)]
pub struct QuatLinearRotationControllerSection {
    pub name: Idstring,
    pub flags: u32,
    pub unknown_1: u32,
    pub duration: f32,
    pub keyframes: Vec<(f32, Vec4f)>
}

#[derive(Debug, ItemReader)]
pub struct LookAtConstrRotationControllerSection {
    pub name: Idstring,
    pub unknown_1: u32,
    pub section_1: u32,
    pub section_2: u32,
    pub section_3: u32
}

#[derive(Debug, ItemReader)]
pub struct ModelToolHashSection {
    version: u16,

    #[read_as(CountedVec<CountedString<u16>>)]
    strings: Vec<String>
}