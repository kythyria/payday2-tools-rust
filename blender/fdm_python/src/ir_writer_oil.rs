
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use pyo3::{Python, PyAny, intern, PyResult};
use itertools::Itertools;

use pd2tools_rust::formats::oil;
use crate::PyEnv;
use crate::mesh::Mesh;

struct GatheredObject<'py> {
    object: &'py PyAny,
    py_id: u64,
    matrix: vek::Mat4<f32>,
    name: String,
    children: Vec<GatheredObject<'py>>,
    data: GatheredData<'py>
}

#[derive(Clone, Copy)]
enum GatheredData<'py> {
    None,
    Mesh(&'py PyAny),
    Camera(&'py PyAny),
    Light(&'py PyAny),
}

fn gather_object_tree<'py>(env: PyEnv<'py>, object: &'py PyAny) -> GatheredObject<'py> {
    let name = object.getattr(intern!{env.python, "name"}).unwrap();
    let name = name.extract().unwrap();

    let matrix = object.getattr(intern!{env.python,"matrix_local"}).unwrap();
    let matrix = matrix_to_vek(matrix);

    let children: Vec<GatheredObject> = object
        .getattr(intern!{env.python,"children"})
        .unwrap()
        .iter()
        .unwrap()
        .map(|i| i.unwrap())
        .map(|i| gather_object_tree(env, i))
        .collect();

    // If this is an armature, we have to worry about bone-parented and skinned children.
    // Children whose parent_type is BONE are parented to a bone.
    // Children whose parent type is OBJECT but have an Armature Deform modifier are skinned.
    // Children whose parent_type is ARMATURE just act like that.

    let data_type = object.getattr(intern!{env.python, "type"}).unwrap();
    let data_type: &str = data_type.extract().unwrap();
    let data = match data_type {
        "MESH" => GatheredData::Mesh(object.getattr(intern!{env.python, "data"}).unwrap()),
        "LIGHT" => GatheredData::Light(object.getattr(intern!{env.python, "data"}).unwrap()),
        "CAMERA" => GatheredData::Camera(object.getattr(intern!{env.python, "data"}).unwrap()),
        _ => GatheredData::None
    };

    GatheredObject {
        object,
        py_id: env.id(object),
        matrix,
        name,
        children,
        data
    }
}

fn matrix_to_vek(bmat: &PyAny) -> vek::Mat4<f32> {
    eprintln!("m2v");
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

struct FlatObject<'py> {
    object: &'py PyAny,
    matrix: vek::Mat4<f32>,
    name: String,
    chunk_id: u32,
    parent_chunk_id: u32,
    blender_data: GatheredData<'py>,
    data: FlatData<'py>
}

enum FlatData<'py> {
    None,
    Mesh(Mesh<'py>),
    Light(FlatLight),
    Camera(FlatCamera)
}

struct FlatLight;
struct FlatCamera;

#[derive(Default)]
struct FlattenedScene<'py> {
    next_chunkid: u32,

    nodes: Vec<FlatObject<'py>>,
    nodes_by_pyid: HashMap<u64, usize>,
    materials: Vec<Rc<str>>
}
impl<'py> FlattenedScene<'py> {
    fn new() -> Self { FlattenedScene {
        next_chunkid: 1,
        ..Default::default()
    } }
    fn add_object_tree(&mut self, obj: &'py GatheredObject, parent_chunk: u32) {
        let chunk_id = self.next_chunkid;
        self.next_chunkid += 1;
        self.nodes.push(FlatObject {
            object: obj.object,
            matrix: obj.matrix,
            name: obj.name.clone(),
            chunk_id,
            parent_chunk_id: parent_chunk,
            blender_data: obj.data,
            data: FlatData::None
        });
        self.nodes_by_pyid.insert(obj.py_id, self.nodes.len() -1);
        for child in &obj.children {
            self.add_object_tree(child, chunk_id)
        }
    }

    fn populate_object_data(&mut self, env: PyEnv<'py>) {
        let mut mats = HashSet::<Rc<str>>::new();

        for obj in self.nodes.iter_mut() {
            obj.data = match obj.blender_data {
                GatheredData::None => FlatData::None,
                GatheredData::Mesh(d) => {
                    let mesh  = Mesh::from_bpy_object(&env, obj.object, d);
                    mats.extend(mesh.material_names.iter().map(|i| Rc::from(*i)));
                    FlatData::Mesh(mesh)
                },
                GatheredData::Camera(_) => FlatData::Camera(FlatCamera),
                GatheredData::Light(_) => FlatData::Light(FlatLight)
            }
        }

        self.materials = mats.into_iter().collect();
        self.materials.sort();
    }
}

fn mesh_to_oil_geometry(me: &Mesh, material_list: &[Rc<str>]) -> oil::Geometry() {
    todo!()
}

fn flat_scene_to_oilchunks(scene: &FlattenedScene, chunks: &mut Vec<oil::Chunk>) {
    for fo in &scene.nodes {
        chunks.push(oil::Node {
            id: fo.chunk_id,
            name: fo.name.clone(),
            transform: fo.matrix.as_(),
            pivot_transform: vek::Mat4::identity(),
            parent_id: fo.parent_chunk_id,
        }.into());

        match &fo.data {
            FlatData::None => (),
            FlatData::Mesh(m) => chunks.push(mesh_to_oil_geometry(m, &scene.materials).into()),
            FlatData::Light(_) => todo!(),
            FlatData::Camera(_) => todo!(),
        }
    }

    for mat in (&scene.materials).into_iter().enumerate() {
        chunks.push(oil::Material {
            id: scene.next_chunkid,
            name: todo!(),
            parent_id: todo!(),
        }.into());
    }
}

pub fn export(env: PyEnv, output_path: &str, units_per_cm: f32, framerate: f32, object: &PyAny) -> PyResult<()> {
    let object_tree = gather_object_tree(env, object);
    let mut flat_scene = FlattenedScene::new();
    flat_scene.add_object_tree(&object_tree, 0xFFFFFFFF);
    
    let mut chunks = vec! [
        oil::SceneInfo3 {
            start_time: 0.0,
            end_time: 1.0,
            author_tag: "nemo@erehwon.invalid".to_owned(),
            source_filename: "fake.blend".to_owned(),
            scene_type: "default".to_owned()
        }.into(),
        oil::MaterialsXml { xml: String::new() }.into()
    ];
    flat_scene_to_oilchunks(&flat_scene, &mut chunks);
    
    let bytes = oil::chunks_to_bytes(&chunks)?;
    std::fs::write(output_path, &bytes)?;

    Ok(())
}