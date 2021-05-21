//! Final Diesel Model format used in release versions of the game.

use nom::IResult;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::{map, map_res};
use nom::multi::{fill, length_data, length_count};
use nom::number::complete::{le_f32, le_u32, le_u64};
use nom::sequence::{terminated, tuple};

use crate::hashindex::Hash as Idstring;

type Vec3f = vek::Vec3<f32>;
type Mat4f = vek::Mat4<f32>;

struct UnparsedSection<'a> {
    r#type: u32,
    id: u32,
    data: &'a [u8]
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

/// Metadata about the model file. Release Diesel never, AFAIK, actually cares about this.
struct AuthorSection {
    /// Very likely the "scene type" field
    name: Idstring,

    /// Email address of the author. In Overkill/LGL's tools, settable in the exporter settings.
    author_email: String,

    /// Absolute path of the original file.
    source_filename: String,
    unknown_2: u32
}
impl AuthorSection {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Self> {
        let (input, (name, author_email, source_filename, unknown_2)) = tuple((
            map(le_u64, Idstring),
            map(cstring, String::from),
            map(cstring, String::from),
            le_u32
        ))(input)?;
        Ok((input, AuthorSection {
            name, author_email, source_filename, unknown_2
        }))
    }
}

/// Scene object node
///
/// Blender calls this an Object, GLTF calls it a Node. Object3d on its own is an Empty node, just marking a point in
/// space, a joint, or suchlike.
struct Object3dSection {
    name: Idstring,
    animation_controllers: Vec<u32>,
    transform: Mat4f,
    parent: u32
}

impl Object3dSection {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Self> {
        let (input, (name, animation_controllers, transform, parent)) = tuple((
            map(le_u64, Idstring),
            length_count(le_u32, terminated(le_u32, le_u64)),
            matrix_4x4,
            le_u32
        ))(input)?;

        Ok((input, Object3dSection {
            name, animation_controllers, transform, parent
        }))
    }
}

struct Bounds {
    min: Vec3f,
    max: Vec3f,
    radius: f32,
    unknown_13: u32
}
impl Bounds {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Self> {
        let (remain, (min, max, radius, unknown_13)) = tuple((
            vec3f, vec3f, le_f32, le_u32
        ))(input)?;
        Ok((remain, Bounds {
            min, max, radius, unknown_13
        }))
    }
}

struct MeshModel {
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
impl MeshModel {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Self> {
        let (input, (geometry_provider, topology_ip, render_atoms, material_group, lightset, bounds, properties, skinbones)) = tuple((
            le_u32,
            le_u32,
            length_count(le_u32, RenderAtom::parse),
            le_u32,
            le_u32,
            Bounds::parse,
            le_u32,
            le_u32
        ))(input)?;
        Ok((input, MeshModel {
            bounds, geometry_provider, topology_ip, render_atoms, material_group, lightset, properties, skinbones
        }))
    }
}

/// A single draw's worth of geometry
///
/// If you get this wrong Diesel doesn't usually crash but will display nonsense.
struct RenderAtom {
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
impl RenderAtom {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a[u8], Self> {
        let (remain, (base_vertex, triangle_count, base_index, geometry_slice_length, material)) = tuple((
            le_u32, le_u32, le_u32, le_u32, le_u32, 
        ))(input)?;
        return Ok((remain, RenderAtom {
            base_vertex, triangle_count, base_index, geometry_slice_length, material
        }))
    }
}

fn cstring<'a>(input: &'a [u8]) -> IResult<&'a[u8], &'a str> {
    let parser = terminated(take_until("\0"), tag(b"\0"));
    map_res(parser, |i: &[u8]| std::str::from_utf8(i))(input)
}

fn matrix_4x4<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], Mat4f> {
    let mut out: [f32; 16] = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
    let (rest, ()) = fill(le_f32, &mut out)(input)?;
    Ok((rest, Mat4f::from_col_array(out)))
}

fn vec3f<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], Vec3f> {
    let (rest, (x, y, z)) = tuple((le_f32, le_f32, le_f32))(input)?;
    Ok((rest, Vec3f { x, y ,z }))
}