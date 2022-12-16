use std::collections::HashMap;
use std::rc::Rc;
use pyo3::{prelude::*, intern, AsPyPointer};
use crate::{ PyEnv, ExportFlag, ExportFlags, model_ir };
use model_ir::*;

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

fn mat4_from_bpy_matrix(bmat: &PyAny) -> vek::Mat4<f32> {
    let mut floats = [[0f32; 4]; 4];
    for c in 0..4 {
        let col = bmat.get_item(c).unwrap();
        for r in 0..4 {
            let cell = col.get_item(r).unwrap().extract::<f32>().unwrap();
            floats[c][r] = cell;
        }
    }
    vek::Mat4::from_col_arrays(floats)
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

pub fn mesh_from_bpy_mesh(env: &PyEnv, data: &PyAny) -> model_ir::Mesh {
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

    data.call_method0(intern!{env.python, "calc_normals_split"}).unwrap();

    let vertex_groups = vgroups_from_bpy_verts(env, data);

    data.call_method0(intern!{env.python, "calc_tangents"}).unwrap();
    let faceloop_normals = TangentSpace::Tangents(get!(env, data, 'iter "loops")
        .map(|lp| Tangent {
            normal: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "normal")),
            tangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "tangent")),
            bitangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "bitangent"))
        })
        .collect()
    );

    let faceloop_colors = get!(env, data, 'iter "vertex_colors")
        .map(|vc|{
            let name: String = get!(env, vc, 'attr "name");
            let cols: Vec<Vec4f> = get!(env, vc, 'iter "data")
                .map(|i| from_bpy_array(get!(env, i, 'attr "color")))
                .collect();
            (name, cols)
        })
        .collect();

    let faceloop_uvs = get!(env, data, 'iter "uv_layers")
        .map(|uvl| {
            let name: String = get!(env, uvl, 'attr "name");
            let uvs: Vec<Vec2f> = get!(env, uvl, 'iter "data")
                .map(|uv| vek2f_from_bpy_vec(env, get!(env, uv, 'attr "uv")))
                .collect();
            (name, uvs)
        })
        .collect();

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

pub fn mesh_from_bpy_object(env: &PyEnv, object: &PyAny, data: &PyAny) -> Mesh {
    let mut mesh = mesh_from_bpy_mesh(env, data);

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

fn vgroups_from_bpy_verts(env: &PyEnv, data: &PyAny) -> VertexGroups {
    let bpy_verts = data.getattr(intern!{env.python, "vertices"}).unwrap();
    let vlen = bpy_verts.len().unwrap();

    let mut out = VertexGroups::with_capacity(vlen, 3);
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

fn gather_object_data(env: &PyEnv, object: &PyAny, out: &mut Scene) -> ObjectData {
    match get!(env, object, 'attr "type") {
        "MESH" => ObjectData::Mesh(mesh_from_bpy_object(env, object, get!(env, object, 'attr "data"))),
        "EMPTY" => ObjectData::None,
        _ => todo!()
    }
}

struct SceneBuilder<'py> {
    env: &'py PyEnv<'py>,
    scene: Scene,
    bpy_obj_to_oid: HashMap<*mut pyo3::ffi::PyObject, ObjectKey>,
    oid_to_bpy_parent: HashMap<ObjectKey, *mut pyo3::ffi::PyObject>
}

impl<'py> SceneBuilder<'py> 
{
    fn new(env: &'py PyEnv) -> SceneBuilder<'py> {
        SceneBuilder {
            env,
            scene: Scene::default(),
            bpy_obj_to_oid: HashMap::new(),
            oid_to_bpy_parent: HashMap::new()
        }
    }

    fn set_scale(&mut self, meters_per_unit: f32) { self.scene.meters_per_unit = meters_per_unit }
    fn set_active_object(&mut self, active_object: ObjectKey) {
        self.scene.active_object = Some(active_object)
    }
    
    fn add_bpy_object(&mut self, object: &PyAny) -> ObjectKey {
        let odata = match get!(self.env, object, 'attr "type") {
            "MESH" => {
                let data = get!(self.env, object, 'attr "data");
                ObjectData::Mesh(mesh_from_bpy_object(self.env, object, data))
            },
            "EMPTY" => ObjectData::None,
            _ => todo!()
        };

        let new_obj = Object {
            name: get!(self.env, object, 'attr "name"),
            parent: None,
            children: Vec::new(),
            transform: mat4_from_bpy_matrix(get!(self.env, object, 'attr "matrix_local")),
            in_collections: Vec::new(),
            data: gather_object_data(self.env, object, &mut self.scene),
        };
        let oid = self.scene.objects.insert(new_obj);

        self.bpy_obj_to_oid.insert(object.as_ptr(), oid);
        let parent = object.getattr(intern!{self.env.python, "parent"}).unwrap();
        if !parent.is_none() {
            self.oid_to_bpy_parent.insert(oid, parent.as_ptr());
        }
        
        oid
    }
}

impl From<SceneBuilder<'_>> for Scene {
    fn from(mut build: SceneBuilder) -> Self {
        let mut parent_links = Vec::with_capacity(build.oid_to_bpy_parent.len());

        for oid in build.scene.objects.keys() {
            match build.oid_to_bpy_parent.get(&oid) {
                None => (),
                Some(p) => {
                    let parent_oid = build.bpy_obj_to_oid[p];
                    parent_links.push((oid, parent_oid));
                },
            }
        }

        for (child, parent) in parent_links {
            build.scene.objects[child].parent = Some(parent);
            build.scene.objects[parent].children.push(child);
        }

        build.scene
    }
}

pub fn scene_from_bpy_selected(env: &PyEnv, data: &PyAny, meters_per_unit: f32) -> Scene {
    let mut scene = Scene::default();
    scene.meters_per_unit = meters_per_unit;

    // According to the manual, it's O(len(bpy.data.objects)) to use children or children_recusive
    // so we should do a pair of iterations instead of recursing ourselves
    // specifically once over children_recursive to grab everything,
    // and once over the grabbed objects to fill in the relations.
    //
    // The actual filling in is done in <Scene as From<SceneBuilder>>::from


    let mut scene = SceneBuilder::new(env);
    scene.set_scale(meters_per_unit);

    let active = scene.add_bpy_object(data);
    scene.set_active_object(active);

    for b_obj in get!(env, data, 'iter "children_recursive") {
        scene.add_bpy_object(b_obj);
    }

    scene.into()
}