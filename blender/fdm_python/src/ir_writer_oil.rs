
use std::{collections::{HashMap, HashSet}, io::Write, convert::TryInto};

use pyo3::{IntoPy, Python, Py, PyErr, PyObject, PyAny, types::PyModule, intern, PyResult};

use pd2tools_rust::formats::oil;

struct GatheredObject<'py> {
    object: &'py PyAny,
    py_id: u64,
    matrix: vek::Mat4<f32>,
    name: String,
    children: Vec<GatheredObject<'py>>,
}

#[derive(Clone, Copy)]
pub struct PyEnv<'py> {
    pub python: Python<'py>,
    id_fn: &'py PyAny,
}
impl<'py> PyEnv<'py> {
    pub fn new(python: Python<'py>) -> PyEnv<'py> {
        let builtins = python.import("builtins").unwrap();
        PyEnv {
            python,
            id_fn: builtins.getattr("id").unwrap()
        }
    }
    pub fn id(&self, pyobj: &'py PyAny) -> u64 {
        self.id_fn.call1( (pyobj,) ).unwrap().extract::<u64>().unwrap()
    }
}

fn gather_object_tree<'py>(env: PyEnv<'py>, object: &'py PyAny) -> GatheredObject<'py> {
    eprintln!("[gather_object_tree] entry");
    let name = object.getattr(intern!{env.python, "name"}).unwrap();
    eprintln!("[gather_object_tree] extract name");
    let name = name.extract().unwrap();
    eprintln!("[gather_object_tree] name: {}", name);

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

    GatheredObject {
        object,
        py_id: env.id(object),
        matrix,
        name,
        children,
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
}

struct FlattenedScene<'py> {
    next_chunkid: u32,

    nodes: Vec<FlatObject<'py>>,
    nodes_by_pyid: HashMap<u64, usize>,
    materials: Vec<(u32, String)>,
    material_groups: Vec<Vec<usize>>
}
impl<'py> Default for FlattenedScene<'py> {
    fn default() -> Self {
        FlattenedScene {
            next_chunkid: 1,
            nodes: Default::default(),
            nodes_by_pyid: Default::default(),
            materials: Default::default(),
            material_groups: Default::default()
        }
    }
}
impl<'py> FlattenedScene<'py> {
    fn new() -> Self { Default::default() }
    fn add_object_tree(&mut self, obj: &'py GatheredObject, parent_chunk: u32) {
        let chunk_id = self.next_chunkid;
        self.next_chunkid += 1;
        self.nodes.push(FlatObject {
            object: obj.object,
            matrix: obj.matrix,
            name: obj.name.clone(),
            chunk_id,
            parent_chunk_id: parent_chunk
        });
        self.nodes_by_pyid.insert(obj.py_id, self.nodes.len() -1);
        for child in &obj.children {
            self.add_object_tree(child, chunk_id)
        }
    }
}

fn flat_scene_to_oilchunks(scene: &FlattenedScene, chunks: &mut Vec<oil::Chunk>) {
    for fo in &scene.nodes {
        chunks.push(oil::Node {
            id: fo.chunk_id,
            name: fo.name.clone(),
            transform: fo.matrix.as_(),
            pivot_transform: vek::Mat4::identity(),
            parent_id: fo.parent_chunk_id,
        }.into())
    }
}

pub fn export(env: PyEnv, output_path: &str, units_per_cm: f32, framerate: f32, object: &PyAny) -> PyResult<()> {
    let mut id_counter = 1;

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