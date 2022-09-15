//! Mesh similar to Blender's Mesh struct.

use std::rc::Rc;

use pyo3::{prelude::*, intern};

use crate::PyEnv;

type Vec3f = vek::Vec3<f32>;

pub struct Mesh<'names> {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<(usize, usize)>,
    pub loops: Vec<Loop>,
    pub triangles: Vec<Triangle>,
    
    pub vertex_group_names: Vec<&'names str>,
    pub material_names: Vec<&'names str>
}

pub struct Vertex {
    pub co: Vec3f,
    pub weights: Vec<(usize, f32)>,
    pub normal: Vec3f
}

pub struct Loop {
    pub vertex: usize,
    pub edge: usize,
    pub normal: Vec3f,
    pub tangent: Vec3f,
    pub bitangent_sign: f32
}

pub struct Triangle {
    pub vertices: [usize; 3],

    /// Faceloops whose properties are relevant to this triangle.
    /// 
    /// AFAIK you have to ignore the connectivity on the referenced loops and just
    /// use the vertex-like properties.
    pub loops: [usize; 3],
    pub material: usize,
}

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

impl<'name> Mesh<'name> {
    pub fn from_bpy_mesh<'py>(env: &PyEnv, data: &'py PyAny) -> Mesh<'py> {
        let vertices = get!(env, data, 'iter "vertices")
            .map(|pv|{
                let weights = get!(env, pv, 'iter "weights")
                    .map(|pvw|(
                        get!(env, pvw, 'attr "group"),
                        get!(env, pvw, 'attr "weight")
                    ))
                    .collect::<Vec<(usize, f32)>>();

                Vertex {
                    co: vek3f_from_tuple(get!(env, pv, 'attr "co")),
                    normal: vek3f_from_tuple(get!(env, pv, 'attr "normal")),
                    weights
                }
            })
            .collect::<Vec<Vertex>>();
        
        let edges = get!(env, data, 'iter "edges").map(|pe|{
            get!(env, pe, 'attr "vertices")
        }).collect::<Vec<(usize, usize)>>();

        let loops = get!(env, data, 'iter "loops")
            .map(|lp|{
                Loop {
                    vertex: get!(env, lp, 'attr "vertex_index"),
                    edge: get!(env, lp, 'attr "edge_index"),
                    normal: vek3f_from_tuple(get!(env, lp, 'attr "normal")),
                    tangent: vek3f_from_tuple(get!(env, lp, 'attr "tangent")),
                    bitangent_sign: get!(env, lp, 'attr "bitangent_sign")
                }
            })
            .collect::<Vec<Loop>>();

        let triangles = get!(env, data, 'iter "loop_triangles")
            .map(|lt|{
                Triangle {
                    vertices: get!(env, lt, 'attr "vertices"),
                    loops: get!(env, lt, 'attr "loops"),
                    material: get!(env, lt, 'attr "material_index"),
                }
            }).collect::<Vec<Triangle>>();
        
        let material_names = get!(env, data, 'iter "materials")
            .map(|mat| {
                get!(env, mat, 'attr "name")
            }).collect::<Vec<&str>>();
        
        Mesh {
            vertices,
            edges,
            loops,
            triangles,
            vertex_group_names: Vec::new(),
            material_names,
        }
    }

    pub fn from_bpy_object<'py>(env: &PyEnv, object: &'py PyAny, data: &'py PyAny) -> Mesh<'py> {
        let mut mesh = Mesh::from_bpy_mesh(env, data);

        mesh.material_names = get!(env, object, 'iter "material_slots").map(|slot| {
            get!(env, slot, 'attr "name")
        }).collect();

        mesh.vertex_group_names = get!(env, object, 'iter "vertex_groups").map(|group|{
            get!(env, group, 'attr "name")
        }).collect();

        mesh
    }
}

fn vek3f_from_tuple(inp: (f32, f32, f32)) -> Vec3f {
    inp.into()
}