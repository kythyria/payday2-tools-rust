use std::convert::TryInto;

use pyo3::{PyAny, PyResult};

use pd2tools_rust::formats::oil;
use slotmap::SecondaryMap;
use crate::PyEnv;
use crate::model_ir::{Mesh, MaterialKey, Scene, ObjectData};

struct MaterialCollector<'s> {
    scene: &'s Scene,
    next_id: u32,
    collected: Vec<oil::Material>,
    solo_mats: SecondaryMap<MaterialKey, u32> 
}
impl<'s> MaterialCollector<'s> {
    fn new(scene: &'s Scene, next_id: u32) -> Self {
        MaterialCollector {
            scene,
            next_id,
            collected: Vec::new(),
            solo_mats: SecondaryMap::new()
        }
    }

    fn append_material(&mut self, name: String, parent_id: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.collected.push(oil::Material { id, name, parent_id });
        id
    }

    /// Take a mesh's material names, and return the ID of the mesh-wide material along with
    /// the mapping from mesh-local material index to material ID.
    fn collect_and_map(&mut self, mat_refs: &[Option<MaterialKey>]) -> (u32, Vec<u32>) {
        let mut mapping = Vec::new();

        if mat_refs.len() == 0 {
            return (0xFFFFFFFFu32, vec![0xFFFFFFFFu32]);
        }
        else if mat_refs.len() == 1 {
            if mat_refs[0].is_none() {
                return (0xFFFFFFFFu32, vec![0xFFFFFFFFu32]);
            }
            
            let matkey = mat_refs[0].clone().unwrap();
            if let Some(id) = self.solo_mats.get(matkey) {
                return (*id, vec![*id]);
            }
            else {
                let mat_name = self.scene.materials[matkey].name.clone();
                let id = self.append_material(mat_name, 0xFFFFFFFFu32);
                self.solo_mats.insert(matkey, id);
                return (id, vec![id]);
            }
        }

        let parent_id = self.append_material("MultiMaterial".into(), 0xFFFFFFFFu32);
        let mut mats = SecondaryMap::<MaterialKey, u32>::new();
        for n in mat_refs {
            if let Some(n) = n {
                use slotmap::secondary::Entry;
                let name = self.scene.materials[*n].name.clone();
                let id = match mats.entry(*n) {
                    None => panic!("How did the material vanish mid-borrow?"),
                    Some(Entry::Occupied(o)) => *o.get(),
                    Some(Entry::Vacant(v)) => *v.insert(self.append_material(name, parent_id))
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

fn mesh_to_oil_geometry(node_id: u32, me: &Mesh, materials: &mut MaterialCollector) -> oil::Geometry {
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

    let norms = me.tangents.iter().map(|i| i.normal)
        .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
        .collect::<Vec<_>>();
    let tangs = me.tangents.iter().map(|i| i.tangent)
        .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
        .collect::<Vec<_>>();
    let binorms = me.tangents.iter().map(|i| i.bitangent)
        .map(|i| i.map(|j| <f32 as Into<f64>>::into(j)))
        .collect::<Vec<_>>();

    og.channels.push(oil::GeometryChannel::Normal(0, norms));
    og.channels.push(oil::GeometryChannel::Tangent(0, tangs));
    og.channels.push(oil::GeometryChannel::Binormal(0, binorms));

    let (root_material, material_mapping) = materials.collect_and_map(&me.material_ids);
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

        // normal/tangent/binormal
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

        og.faces.push(oil::GeometryFace {
            material_id: material_mapping[local_mat_id],
            smoothing_group: 0, // TODO: Does Blender *have* smoothing groups and do we care?
            loops,
        });
    }

    og
}

fn scene_to_oilchunks(scene: &crate::model_ir::Scene, chunks: &mut Vec<oil::Chunk>) {
    let base_chunkid = 1u32;
    let base_mat_chunkid = (base_chunkid as usize + scene.objects.len()).try_into().unwrap();
    let mut mat_collector = MaterialCollector::new(scene, base_mat_chunkid);

    let chunkid_for_object = scene.objects.keys()
        .enumerate()
        .map(|(idx, key)| (key, idx as u32 + base_chunkid))
        .collect::<slotmap::SecondaryMap<_,_>>();

    for (oid, obj) in &scene.objects {
        let parent_id = obj.parent
            .map_or( 0xFFFFFFFFu32, |p| chunkid_for_object[p]);
        
        let chunk_id = chunkid_for_object[oid];
        chunks.push(oil::Node {
            id: chunk_id,
            name: obj.name.clone(),
            transform: obj.transform.as_(),
            pivot_transform: vek::Mat4::identity(),
            parent_id,
            
        }.into());

        match &obj.data {
            ObjectData::None => (),
            ObjectData::Armature => (),
            ObjectData::Bone => (),
            ObjectData::Mesh(md) => {
                let ch = mesh_to_oil_geometry(chunk_id, md, &mut mat_collector);
                chunks.push(ch.into())
            },
            ObjectData::Light(_) => todo!(),
            ObjectData::Camera(_) => todo!(),
        }
    }
}

pub fn export(env: PyEnv, output_path: &str, meters_per_unit: f32, framerate: f32, object: &PyAny) -> PyResult<()> {
    let scene = crate::ir_blender::scene_from_bpy_selected(&env, object, meters_per_unit);
    let mut chunks = vec! [
        oil::SceneInfo3 {
            start_time: 0.0,
            end_time: 1.0,
            author_tag: "nemo@erehwon.invalid".to_owned(),
            source_filename: scene.source_file.clone(),
            scene_type: "default".to_owned()
        }.into(),
        oil::MaterialsXml { xml: String::new() }.into()
    ];
    scene_to_oilchunks(&scene, &mut chunks);
    let bytes = oil::chunks_to_bytes(&chunks)?;
    std::fs::write(output_path, &bytes)?;
    Ok(())
}