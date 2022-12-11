
use std::collections::{HashMap, hash_map::Entry, HashSet};
use std::convert::TryInto;
use std::rc::Rc;

use pyo3::{PyAny, intern, PyResult};

use pd2tools_rust::formats::oil;
use crate::PyEnv;
use crate::mesh2::{ Mesh, ExportFlag as MeshExportFlag };

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

struct MaterialCollector {
    next_id: u32,
    collected: Vec<oil::Material>,
    solo_mats: HashMap<Rc<str>, u32> 
}
impl MaterialCollector {
    fn append_material(&mut self, name: String, parent_id: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.collected.push(oil::Material { id, name, parent_id });
        id
    }

    /// Take a mesh's material names, and return the ID of the mesh-wide material along with
    /// the mapping from mesh-local material index to material ID.
    fn collect_and_map(&mut self, names: &[Option<Rc<str>>]) -> (u32, Vec<u32>) {
        let mut mapping = Vec::new();

        if names.len() == 0 {
            return (0xFFFFFFFFu32, vec![0xFFFFFFFFu32]);
        }
        else if names.len() == 1 {
            if names[0].is_none() {
                return (0xFFFFFFFFu32, vec![0xFFFFFFFFu32]);
            }
            
            let name = names[0].clone().unwrap();
            if let Some(id) = self.solo_mats.get(&name) {
                return (*id, vec![*id]);
            }
            else {
                let id = self.append_material(name.to_string(), 0xFFFFFFFFu32);
                self.solo_mats.insert(name, id);
                return (id, vec![id]);
            }
        }

        let parent_id = self.append_material("MultiMaterial".into(), 0xFFFFFFFFu32);
        let mut mats = HashMap::<&str, u32>::new();
        for n in names {
            if let Some(n) = n {
                let id = match mats.entry(n.as_ref()) {
                    Entry::Occupied(o) => *o.get(),
                    Entry::Vacant(v) => *v.insert(self.append_material(n.to_string(), parent_id))
                };
                mapping.push(id);
            }
            else {
                mapping.push(0xFFFFFFFFu32);
            }
        }

        (parent_id, mapping)
    }
}

struct FlatObject<'py> {
    object: &'py PyAny,
    matrix: vek::Mat4<f32>,
    name: String,
    chunk_id: u32,
    parent_chunk_id: u32,
    blender_data: GatheredData<'py>,
    data: FlatData
}

enum FlatData {
    None,
    Mesh(Mesh),
    Light(FlatLight),
    Camera(FlatCamera)
}

struct FlatLight;
struct FlatCamera;

#[derive(Default)]
struct FlattenedScene<'py> {
    next_chunkid: u32,

    nodes: Vec<FlatObject<'py>>,
    nodes_by_pyid: HashMap<u64, usize>
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
        for obj in self.nodes.iter_mut() {
            obj.data = match obj.blender_data {
                GatheredData::None => FlatData::None,
                GatheredData::Mesh(d) => {
                    use MeshExportFlag::*;
                    let mesh  = Mesh::from_bpy_object(&env, obj.object, d,
                        Normals | Tangents | TexCoords | Colors | Weights
                    );
                    FlatData::Mesh(mesh)
                },
                GatheredData::Camera(_) => FlatData::Camera(FlatCamera),
                GatheredData::Light(_) => FlatData::Light(FlatLight)
            }
        }
    }
}

fn mesh_to_oil_geometry(node_id: u32, me: &Mesh, material_id_base: u32, materials: &mut MaterialCollector) -> oil::Geometry {
    let mut og = oil::Geometry {
        node_id,
        material_id: 0xFFFFFFFFu32,
        casts_shadows: true,
        receives_shadows: true,
        channels: Vec::with_capacity(5),
        faces: Vec::with_capacity(me.triangles.len()),
        skin: None,
        override_bounding_box: None,
    };

    // TODO: Do we care about duplication? Is this horrifyingly slow?
    // TODO: Does the OIL->FDM step *care* about if there are unused things?

    og.channels.push(oil::GeometryChannel::Position(0, me.vertices.iter().map(|i|{
        i.map(|c| c.into())
    }).collect()));

    let mut uv_list = me.faceloop_uvs.iter().collect::<Vec<_>>();
    uv_list.sort_by(|i,j| i.0.cmp(j.0));

    for (idx, (_name, tc)) in uv_list.into_iter().enumerate() {
        let data = tc.iter().map(|i| i.map(|j| j.into())).collect();
        og.channels.push(oil::GeometryChannel::TexCoord(idx as u32 + 1, data))
    }

    let mut vc_list = me.faceloop_colors.iter().collect::<Vec<_>>();
    vc_list.sort_by(|i,j| i.0.cmp(j.0));

    for (idx, (_name, vc)) in vc_list.iter().enumerate() {
        let data = vc.iter().map(|i| {
            let v: vek::Vec4<f64> = i.map(|j| j.into());
            let c = vek::Rgba::from(v);
            c.rgb()
        }).collect();
        og.channels.push(oil::GeometryChannel::Colour(idx as u32 + 1, data))
    }

    match &me.faceloop_normals {
        crate::mesh2::TangentSpace::None => (),
        crate::mesh2::TangentSpace::Normals(normals) => {
            let data = normals.iter().map(|i| i.map(|j| j.into())).collect();
            og.channels.push(oil::GeometryChannel::Normal(0, data));
        },
        crate::mesh2::TangentSpace::Tangents(tangents) => {
            let norms = tangents.iter().map(|i| i.normal)
                .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
                .collect::<Vec<_>>();
            let tangs = tangents.iter().map(|i| i.tangent)
                .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
                .collect::<Vec<_>>();
            let binorms = tangents.iter().map(|i| i.bitangent)
                .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
                .collect::<Vec<_>>();

            og.channels.push(oil::GeometryChannel::Normal(0, norms));
            og.channels.push(oil::GeometryChannel::Tangent(0, tangs));
            og.channels.push(oil::GeometryChannel::Binormal(0, binorms));
        },
    };

    let (root_material, material_mapping) = materials.collect_and_map(&me.material_names);
    og.material_id = root_material;

    for tri in &me.triangles {
        let local_mat_id = me.polygons[tri.polygon].material;
        let mut loops = Vec::with_capacity(5);
        let mut channel = 0;

        loops.push(oil::GeometryFaceloop {
            channel,
            a: me.faceloops[tri.loops[0]].vertex as u32,
            b: me.faceloops[tri.loops[1]].vertex as u32,
            c: me.faceloops[tri.loops[2]].vertex as u32
        });

        for _ in 0..me.faceloop_uvs.len() {
            channel += 1;
            loops.push(oil::GeometryFaceloop {
                channel,
                a: tri.loops[0] as u32,
                b: tri.loops[1] as u32,
                c: tri.loops[2] as u32
            })
        }

        for _ in 0..me.faceloop_colors.len() {
            channel += 1;
            loops.push(oil::GeometryFaceloop {
                channel,
                a: tri.loops[0] as u32,
                b: tri.loops[1] as u32,
                c: tri.loops[2] as u32
            })
        }

        match &me.faceloop_normals {
            crate::mesh2::TangentSpace::None => (),
            crate::mesh2::TangentSpace::Normals(_) => {
                channel += 1;
                loops.push(oil::GeometryFaceloop {
                    channel,
                    a: tri.loops[0] as u32,
                    b: tri.loops[1] as u32,
                    c: tri.loops[2] as u32
                });
            },
            crate::mesh2::TangentSpace::Tangents(_) => {
                channel += 1;
                loops.push(oil::GeometryFaceloop {
                    channel,
                    a: tri.loops[0] as u32,
                    b: tri.loops[1] as u32,
                    c: tri.loops[2] as u32
                });
                channel += 1;
                loops.push(oil::GeometryFaceloop {
                    channel,
                    a: tri.loops[0] as u32,
                    b: tri.loops[1] as u32,
                    c: tri.loops[2] as u32
                });
                channel += 1;
                loops.push(oil::GeometryFaceloop {
                    channel,
                    a: tri.loops[0] as u32,
                    b: tri.loops[1] as u32,
                    c: tri.loops[2] as u32
                });
            }
        }

        og.faces.push(oil::GeometryFace {
            material_id: material_mapping[local_mat_id],
            smoothing_group: 0, // TODO: Does Blender *have* smoothing groups and do we care?
            loops,
        });
    }

    og
}

fn flat_scene_to_oilchunks(scene: &FlattenedScene, chunks: &mut Vec<oil::Chunk>) {  
    let mut mat_collector = MaterialCollector {
        next_id: scene.next_chunkid,
        collected: Default::default(),
        solo_mats: Default::default()
    };
    
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
            FlatData::Mesh(m) => {
                let ch = mesh_to_oil_geometry(fo.chunk_id, m, 0, &mut mat_collector);
                chunks.push(ch.into())
            },
            FlatData::Light(_) => todo!(),
            FlatData::Camera(_) => todo!(),
        }
    }

    chunks.extend(mat_collector.collected.drain(..).map(|i| i.into()));
}

pub fn export(env: PyEnv, output_path: &str, units_per_cm: f32, framerate: f32, object: &PyAny) -> PyResult<()> {
    let object_tree = gather_object_tree(env, object);
    let mut flat_scene = FlattenedScene::new();
    flat_scene.add_object_tree(&object_tree, 0xFFFFFFFF);
    flat_scene.populate_object_data(env);
    
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