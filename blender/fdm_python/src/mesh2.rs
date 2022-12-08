use std::collections::HashMap;
use std::rc::Rc;
use pyo3::{prelude::*, intern};
use crate::PyEnv;

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;

macro_rules! get {
    ($env:expr, $ob:expr, 'attr $field:literal) => {
        $ob.getattr(intern!{$env.python, $field}).unwrap().extract().unwrap()
    };
    ($env:expr, $ob:expr, 'iter $field:literal) => {
        $ob.getattr(intern!{$env.python, $field})
            .unwrap()
            .iter()
            .unwrap()
            .map(Result::unwrap)
    }
}

pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub edges: Vec<(usize, usize)>,
    pub faceloops: Vec<Faceloop>,
    pub polygons: Vec<Polygon>,
    pub triangles: Vec<Triangle>,

    pub vertex_groups: VertexGroups,

    pub faceloop_normals: TangentSpace,
    pub faceloop_colors: HashMap<String, Vec<Vec4f>>,
    pub faceloop_uvs: HashMap<String, Vec<Vec2f>>,

    pub material_names: Vec<Option<Rc<str>>>,
}

pub struct Faceloop {
    pub vertex: usize,
    pub edge: usize
}

pub struct Weight {
    pub group: usize,
    pub weight: f32
}

pub struct Polygon {
    pub base: usize,
    pub count: usize,
    pub material: usize
}

pub struct Triangle {
    pub loops: [usize; 3],
    pub polygon: usize,
}

pub enum TangentSpace {
    None,
    Normals(Vec<Vec3f>),
    Tangents(Vec<Tangent>)
}

pub struct Tangent {
    pub normal: Vec3f,
    pub tangent: Vec3f,
    pub bitangent: Vec3f
}

#[derive(Clone, Copy)]
pub struct BaseCount {
    pub base: usize,
    pub count: usize
}
impl From<&BaseCount> for std::ops::Range<usize> {
    fn from(inp: &BaseCount) -> Self {
        std::ops::Range {
            start: inp.base,
            end: inp.base + inp.count,
        }
    }
}

#[derive(Default)]
pub struct VertexGroups {
    pub weights: Vec<Weight>,
    pub vertices: Vec<BaseCount>,
    pub names: Vec<String>
}

impl std::ops::Index<usize> for VertexGroups {
    type Output = [Weight];

    fn index(&self, index: usize) -> &Self::Output {
        self.vertices.as_slice().get(index)
            .map(|bc| {
                let r: std::ops::Range<usize> = bc.into();
                &self.weights[r]
            })
            .unwrap_or(&[])
    }
}
impl VertexGroups {
    pub fn with_capacity(vtx_count: usize, weight_count: usize) -> VertexGroups {
        VertexGroups {
            weights: Vec::with_capacity(vtx_count * weight_count),
            vertices: Vec::with_capacity(vtx_count),
            names: Vec::new()
        }
    }
    pub fn has_weights(&self) -> bool { !self.vertices.is_empty() }
    pub fn is_empty(&self) -> bool { self.vertices.is_empty() }
    pub fn add_for_vertex(&mut self, groups: impl Iterator<Item=Weight>) {
        let base = self.weights.len();
        self.weights.extend(groups);
        let count = self.weights.len() - base;
        self.vertices.push(BaseCount { base, count })
    }

    pub fn from_bpy_verts(env: &PyEnv, data: &PyAny) -> VertexGroups {
        let bpy_verts = data.getattr(intern!{env.python, "vertices"}).unwrap();
        let vlen = bpy_verts.len().unwrap();

        let mut out = Self::with_capacity(vlen, 3);
        for bv in bpy_verts.iter().unwrap() {
            let bv = bv.unwrap();
            let groups = get!(env, bv, 'iter "groups")
                .map(|grp| Weight {
                    group: get!(env, grp, 'attr "group"),
                    weight: get!(env, grp, 'attr "weight"),
                });
            out.add_for_vertex(groups)
        }
        out
    }
}

fn vek2f_from_tuple(inp: (f32, f32)) -> Vec2f {
    inp.into()
}

fn vek3f_from_tuple(inp: (f32, f32, f32)) -> Vec3f {
    inp.into()
}

fn vek2f_from_bpy_vec(env: &PyEnv, data: &PyAny) -> Vec2f {
    let tuple = data.call_method0(intern!(env.python, "to_tuple")).unwrap().extract().unwrap();
    vek2f_from_tuple(tuple)
}

fn vek3f_from_bpy_vec(env: &PyEnv, data: &PyAny) -> Vec3f {
    let tuple = data.call_method0(intern!(env.python, "to_tuple")).unwrap().extract().unwrap();
    vek3f_from_tuple(tuple)
}

fn from_bpy_array<const N:usize,T,E>(data: &PyAny) -> T
where
    T: From<[E; N]>,
    E: Default + Copy + for<'a> FromPyObject<'a>
{
    let mut a: [E; N] = [E::default(); N];
    for i in 0..N {
        a[i] = data.get_item(i).unwrap().extract().unwrap();
    }
    T::from(a)
}

#[enumflags2::bitflags]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum ExportFlag {
    Normals,
    Tangents,
    TexCoords,
    Colors,
    Weights
}
type ExportFlags = enumflags2::BitFlags<ExportFlag>;

impl Mesh {
    pub fn from_bpy_mesh(env: &PyEnv, data: &PyAny, flags: ExportFlags) -> Mesh {
        let vertices = get!(env, data, 'iter "vertices")
            .map(|vtx| vek3f_from_bpy_vec(env, get!(env, vtx, 'attr "co")))
            .collect();

        //let edges = get!(env, data, 'iter "edges")
        //    .map(|ed| get!(env, ed, 'attr "vertices"))
        //    .collect();

        let faceloops = get!(env, data, 'iter "loops")
            .map(|lp| Faceloop {
                vertex: get!(env, lp, 'attr "vertex_index"),
                edge: get!(env, lp, 'attr "edge_index")
            })
            .collect();
        
        let polygons = get!(env, data, 'iter "polygons")
            .map(|poly|{
                Polygon {
                    base: get!(env, poly, 'attr "loop_start"),
                    count: get!(env, poly, 'attr "loop_total"),
                    material: get!(env, poly, 'attr "material_index"),
                }
            })
            .collect();

        data.call_method0(intern!{env.python, "calc_loop_triangles"}).unwrap();
        let triangles = get!(env, data, 'iter "loop_triangles")
            .map(|tri| Triangle {
                loops: from_bpy_array(get!(env, tri, 'attr "loops")),
                polygon: get!(env, tri, 'attr "polygon_index"),
            })
            .collect();

        if flags.contains(ExportFlag::Normals) {
            data.call_method0(intern!{env.python, "calc_normals_split"}).unwrap();
        }

        let vertex_groups = if flags.contains(ExportFlag::Weights) {
            VertexGroups::from_bpy_verts(env, data)
        }
        else {
            VertexGroups::default()
        };

        let faceloop_normals = if flags.contains(ExportFlag::Normals | ExportFlag::Tangents) {
            data.call_method0(intern!{env.python, "calc_tangents"}).unwrap();
            TangentSpace::Tangents(get!(env, data, 'iter "loops")
                .map(|lp| Tangent {
                    normal: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "normal")),
                    tangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "tangent")),
                    bitangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "bitangent"))
                })
                .collect()
            )
        }
        else if flags.contains(ExportFlag::Normals) {
            TangentSpace::Normals(get!(env, data, 'iter "loops")
                .map(|lp| vek3f_from_bpy_vec(env, get!(env, lp, 'attr "normal")))
                .collect()
            )
        }
        else {
            TangentSpace::None
        };

        let faceloop_colors = if flags.contains(ExportFlag::Colors) {
            get!(env, data, 'iter "vertex_colors")
                .map(|vc|{
                    let name: String = get!(env, vc, 'attr "name");
                    let cols: Vec<Vec4f> = get!(env, vc, 'iter "data")
                        .map(|i| from_bpy_array(get!(env, i, 'attr "color")))
                        .collect();
                    (name, cols)
                })
                .collect()
        }
        else {
            HashMap::new()
        };

        let faceloop_uvs = if flags.contains(ExportFlag::TexCoords) {
            get!(env, data, 'iter "uv_layers")
                .map(|uvl| {
                    let name: String = get!(env, uvl, 'attr "name");
                    let uvs: Vec<Vec2f> = get!(env, uvl, 'iter "data")
                        .map(|uv| vek2f_from_bpy_vec(env, get!(env, uv, 'attr "uv")))
                        .collect();
                    (name, uvs)
                })
                .collect()
        }
        else {
            HashMap::new()
        };

        let material_names = get!(env, data, 'iter "materials")
            .map(|mat| {
                if mat.is_none() { return None }
                let st: String = get!(env, mat, 'attr "name");
                Some(Rc::from(st))
            })
            .collect();

        Mesh {
            vertices,
            edges: Vec::new(),
            faceloops,
            polygons,
            triangles,
            vertex_groups,
            faceloop_normals,
            faceloop_colors,
            faceloop_uvs,
            material_names,
        }
    }

    pub fn from_bpy_object(env: &PyEnv, object: &PyAny, data: &PyAny, flags: ExportFlags) -> Mesh {
        let mut mesh = Self::from_bpy_mesh(env, data, flags);

        mesh.material_names.clear();
        mesh.material_names.extend(
            get!(env, object, 'iter "material_slots")
            .map(|ms| get!(env, ms, 'attr "material"))
            .map(|mat: &PyAny| {
                if mat.is_none() { return None }
                let st: String = get!(env, mat, 'attr "name");
                Some(Rc::from(st))
            })
        );

        mesh.vertex_groups.names = get!(env, object, 'iter "vertex_groups")
            .map(|vg| get!(env, vg, 'attr "name"))
            .collect();

        mesh
    }
}