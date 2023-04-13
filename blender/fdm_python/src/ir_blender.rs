use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use pyo3::{prelude::*, intern};
use vek::Mat4;
use crate::bpy::{PropCollection, WrapsPyAny};
use crate::{ PyEnv, model_ir, bpy };
use model_ir::*;

type Vec3f = vek::Vec3<f32>;
type Transform = vek::Transform<f32, f32, f32>;

type PyObjPtr = *mut pyo3::ffi::PyObject;

macro_rules! get {
    ($ob:expr, 'attr $field:literal) => {
        $ob.getattr(intern!{$ob.py(), $field}).unwrap().extract().unwrap()
    };
    ($ob:expr, 'iter $field:literal) => {
        $ob.getattr(intern!{$ob.py(), $field})
            .unwrap()
            .iter()
            .unwrap()
            .map(Result::unwrap)
    };
}

fn mesh_from_bpy_mesh(data: bpy::Mesh) -> model_ir::Mesh {
    let vertices = data.iter_vertices()
        .map(|vtx| vtx.co())
        .collect();

    //let edges = get!(env, data, 'iter "edges")
    //    .map(|ed| get!(env, ed, 'attr "vertices"))
    //    .collect();

    let faceloops = data.loops().iter()
        .map(|lp| Faceloop {
            vertex: lp.vertex_index(),
            edge: lp.edge_index()
        })
        .collect();
    
    let polygons = data.polygons().iter()
        .map(|poly|{
            Polygon {
                base: poly.loop_start(),
                count: poly.loop_total(),
                material: poly.material_index(),
            }
        })
        .collect();

    data.calc_loop_triangles();
    let triangles = data.loop_triangles().iter()
        .map(|tri| Triangle {
            loops: tri.loops(),
            polygon: tri.polygon_index()
        })
        .collect();

    data.calc_normals_split();

    let vertex_groups = vgroups_from_bpy_verts(&data);

    let mut vertex_colors = BTreeMap::new();
    let mut faceloop_colors = BTreeMap::new();
    let mut faceloop_uvs = BTreeMap::new();

    for att in data.attributes() {
        use bpy::{AttributeDomain as AD, AttributeType as AT};
        match (att.domain(), att.data_type()) {
            (AD::Point, AT::ByteColor) |
            (AD::Point, AT::FloatColor) => {
                let data = att.f32_color_data().iter().map(|i| i.value()).collect();
                vertex_colors.insert(att.name().to_owned(), data);
            },
            (AD::Faceloop, AT::ByteColor) |
            (AD::Faceloop, AT::FloatColor) => {
                let data = att.f32_color_data().iter().map(|i| i.value()).collect();
                faceloop_colors.insert(att.name().to_owned(), data);
            },
            (_,_) => continue
        };
    }

    for uv in data.uv_layers() {
        faceloop_uvs.insert(uv.name().into(), uv.uv().iter().map(|i| i.value()).collect());
    }

    let tangents = if faceloop_uvs.is_empty() { // and 
        TangentLayer::Normals(data.loops().iter()
            .map(|lp| lp.normal())
            .collect()
        )
    }
    else { // We have tangents!
        TangentLayer::Tangents(data.loops().iter()
            .map(|lp| Tangent {
                normal: lp.normal(),
                tangent: lp.tangent(),
                bitangent: lp.bitangent(),
            })
            .collect()
        )
    };

    let mut diesel = DieselMeshSettings::default();
    let bpy_diesel = data.diesel_settings();
    if !bpy_diesel.is_none() {
        diesel.cast_shadows = get!(bpy_diesel, 'attr "cast_shadows");
        diesel.receive_shadows = get!(bpy_diesel, 'attr "receive_shadows");
        diesel.bounds_only = get!(bpy_diesel, 'attr "bounds_only");
    }

    Mesh {
        vertices,
        edges: Vec::new(),
        faceloops,
        polygons,
        triangles,
        vertex_groups,
        vertex_colors,
        tangents,
        faceloop_colors,
        faceloop_uvs,
        material_names: Vec::new(),
        material_ids: Vec::new(),
        diesel,
        skin: None
    }
}

fn vgroups_from_bpy_verts(data: &bpy::Mesh) -> VertexGroups {
    let bpy_verts = data.vertices();
    let vlen = bpy_verts.len();

    let mut out = VertexGroups::with_capacity(vlen, 3);
    for bv in bpy_verts.iter() {
        let groups = bv.groups().into_iter()
            .map(|grp| Weight {
                group: grp.group(),
                weight: grp.weight(),
            });
        out.add_for_vertex(groups)
    }
    out
}

fn armature_modifiers_of<'py>(object: &'py bpy::Object<'py>) -> impl Iterator<Item=bpy::ArmatureModifier> {
    object.iter_modifiers().filter_map(|mo| mo.try_into_armature())
}

/// Wrapper for an evaluated, triangulated mesh that frees it automatically
/// 
/// Presumably io_scene_gltf does this sort of thing because otherwise the temp meshes would
/// pile up dangerously fast? IDK, but it does it so we're cargo culting. We also borrow the
/// way it disables armatures, there's probably a better way.
struct TemporaryMesh<'py> {
    mesh: bpy::Mesh<'py>,
    object: bpy::Object<'py>,
}
impl<'py> TemporaryMesh<'py> {
    /// Get the evaluated mesh for an object
    /// 
    /// We disable and re-enable any armature modifiers so that skinning doesn't have an
    /// effect here.
    fn from_depgraph(env: &'py PyEnv, object: &bpy::Object<'py>) -> TemporaryMesh<'py> {
        //for idx, modifier in enumerate(blender_object.modifiers):
        //if modifier.type == 'ARMATURE':
        //    armature_modifiers[idx] = modifier.show_viewport
        //    modifier.show_viewport = False
        let mut armature_modifiers = Vec::<(bpy::ArmatureModifier, bool)>::new();
        for mo in armature_modifiers_of(&object) {
            armature_modifiers.push((mo, mo.show_viewport()));
            mo.set_show_viewport(false);
        }

        let depsgraph = env.b_c_evaluated_depsgraph_get().unwrap();
        let evaluated_obj = object.evaluated_get(depsgraph);
        let mesh = evaluated_obj.to_mesh(true, depsgraph);

        if mesh.as_pyany().getattr(intern!(mesh.py(), "uv_layers")).unwrap().len().unwrap() > 0 {
            // Calculate the tangents here, because this can fail if the mesh still has ngons,
            // and this is where we make a new mesh anyway
            match mesh.calc_tangents() {
                Ok(_) => (),
                Err(_) => {
                    let bm = bpy::bmesh::new(mesh.py()).unwrap();
                    bm.from_mesh(mesh.as_pyany()).unwrap();
                    let faces = bm.faces().unwrap();
                    env.bmesh_ops.triangulate(&bm, faces).unwrap();
                    bm.to_mesh(mesh.as_pyany()).unwrap();
                    mesh.calc_tangents().unwrap();
                },
            }
        }

        for (mo, vis) in armature_modifiers {
            mo.set_show_viewport(vis);
        }
        
        TemporaryMesh {
            mesh,
            object: evaluated_obj,
        }
    }
}
impl Drop for TemporaryMesh<'_> {
    fn drop(&mut self) {
        match self.object.to_mesh_clear() {
            _ => () // the worst that can happen is we leak memory, I think?
        }
    }
}
impl<'py> std::ops::Deref for TemporaryMesh<'py> {
    type Target = bpy::Mesh<'py>;

    fn deref(&self) -> &Self::Target {
        &self.mesh
    }
}

#[derive(Hash, PartialEq, Eq, Debug)]
enum BpyParent {
    Object(PyObjPtr),
    Bone(PyObjPtr, String)
}

struct SceneBuilder<'py> {
    env: &'py PyEnv<'py>,
    scene: Scene,
    bpy_mat_to_matid: HashMap<PyObjPtr, MaterialKey>,
    child_oid_to_bpy_parent: HashMap<ObjectKey, BpyParent>,
    bpy_parent_to_oid_parent: HashMap<BpyParent, ObjectKey>,
    
    /// (being_skinned, skeleton, model_to_mid)
    skin_requests: Vec<(ObjectKey, PyObjPtr, Mat4<f32>)>
}

impl<'py> SceneBuilder<'py> 
{
    fn new(env: &'py PyEnv) -> SceneBuilder<'py> {
        SceneBuilder {
            env,
            scene: Scene::default(),
            bpy_mat_to_matid: HashMap::new(),
            bpy_parent_to_oid_parent: HashMap::new(),
            child_oid_to_bpy_parent: HashMap::new(),
            skin_requests: Vec::new()
        }
    }

    fn set_scale(&mut self, meters_per_unit: f32) { self.scene.meters_per_unit = meters_per_unit }
    fn set_active_object(&mut self, active_object: ObjectKey) {
        self.scene.active_object = Some(active_object)
    }
    fn set_diesel(&mut self, di: DieselSceneSettings) {
        self.scene.diesel = di;
    }
    
    fn add_bpy_object(&mut self, object: bpy::Object<'py>) -> ObjectKey {
        // If this is an armature, we have to worry about bone-parented and skinned children.
        // Children whose parent_type is BONE are parented to a bone.
        // Children whose parent type is OBJECT but have an Armature Deform modifier are skinned.
        // Children whose parent_type is ARMATURE just act like that.

        let otype = object.r#type();
        let odata = match otype {
            bpy::ObjectType::Mesh => ObjectData::Mesh(self.add_bpy_mesh_instance(&object)),
            bpy::ObjectType::Empty => ObjectData::None,
            bpy::ObjectType::Armature => ObjectData::Armature(self.add_bpy_armature_bones(&object)),
            _ => todo!()
        };

        let new_obj = Object {
            name: object.name().into(),
            parent: None,
            children: Vec::new(),
            transform: object.matrix_local(),
            in_collections: Vec::new(),
            data: odata,
            skin_role: if otype == bpy::ObjectType::Armature { SkinRole::Armature } else { SkinRole::None }
        };
        let oid = self.scene.objects.insert(new_obj);

        self.bpy_parent_to_oid_parent.insert(BpyParent::Object(object.as_ptr()), oid);
        let parent = object.parent();
        if let Some(parent) = parent {
            let parent_type = object.parent_type();
            let pkey = match parent_type {
                bpy::ParentType::Object => BpyParent::Object(parent.as_ptr()),
                bpy::ParentType::Bone => {
                    let bone_name = object.parent_bone();
                    if bone_name.len() == 0 {
                        BpyParent::Object(parent.as_ptr())
                    }
                    else {
                        BpyParent::Bone(parent.as_ptr(), bone_name.into())
                    }
                },
                bpy::ParentType::Armature => {
                    let model_to_world = object.matrix_world();
                    self.skin_requests.push((oid, parent.as_ptr(), model_to_world));
                    BpyParent::Object(parent.as_ptr())
                },
                _ => panic!("Unknown parent type {}", parent_type)
            };
            self.child_oid_to_bpy_parent.insert(oid, pkey);
        }
        
        if let Some(mo) = armature_modifiers_of(&object).next() {
            let skel = mo.object();
            let model_to_world = object.matrix_world();
            self.skin_requests.push((oid, skel.as_ptr(), model_to_world));
        }

        oid
    }

    fn add_bpy_mesh_instance(&mut self, object: &bpy::Object<'py>) -> Mesh {
        let data = TemporaryMesh::from_depgraph(self.env, &object);
        let mut mesh = mesh_from_bpy_mesh(*data);

        mesh.vertex_groups.names = object.iter_vertex_groups()
            .map(|vg| vg.name().into() )
            .collect();

        let mats = object.iter_material_slots() 
            .map(|ms| ms.material())
            .collect::<Vec<_>>();

        mesh.material_names.clear();
        mesh.material_names.extend(
            mats.iter()
            .map(|mat| {
                mat.map(|m| Rc::from(m.name()))
            })
        );

        mesh.material_ids.clear();
        mesh.material_ids.extend(
            mats.iter()
            .map(|mat| {
                mat.map(|m| self.add_bpy_material(m))
            })
        );

        mesh
    }

    fn add_bpy_material(&mut self, mat: bpy::Material) -> MaterialKey {
        if self.bpy_mat_to_matid.contains_key(&mat.as_ptr()) {
            return self.bpy_mat_to_matid[&mat.as_ptr()]
        }

        let new_mat = Material {
            name: mat.name().into(),
        };

        self.scene.materials.insert(new_mat)
    }

    fn add_bpy_armature_bones(&mut self, object: &bpy::Object<'py>) -> BindPoseKey {
        let mut joints = Vec::new();

        let data = bpy::Armature::wrap(object.data());
        for bpy_bone in data.iter_bones() {
            // For some reason things being parented to bone tails *isn't* a display trick.
            // Bones really are stored that way.
            // So the position of a bone is its head position plus the parent's tail pos.
            // And the rotation comes from the `matrix` property.
            let bone_name = bpy_bone.name();
            let head = bpy_bone.head();
            let rot= bpy_bone.matrix().to_quaternion();
            let parent = bpy_bone.parent();
            let parent_tail = match &parent {
                None => Vec3f::new(0.0, 0.0, 0.0),
                Some(parent) => parent.tail()
            };

            let transform = Transform {
                position: parent_tail + head,
                orientation: rot,
                scale: vek::Vec3::one()
            };

            let bone_obj = Object {
                name: bone_name.to_owned(),
                parent: None,
                children: Vec::new(),
                transform,
                in_collections: Vec::new(),
                data: ObjectData::None,
                skin_role: SkinRole::Bone,
            };

            let bone_key = self.scene.objects.insert(bone_obj);
            self.bpy_parent_to_oid_parent
                .insert(BpyParent::Bone(object.as_ptr(), bone_name.to_owned()), bone_key);
            match parent {
                None => {
                    self.child_oid_to_bpy_parent
                    .insert(bone_key, BpyParent::Object(object.as_ptr()));
                },
                Some(parent) => {
                    let parent_name = parent.name().to_owned();
                    self.child_oid_to_bpy_parent
                    .insert(bone_key, BpyParent::Bone(object.as_ptr(), parent_name));
                }
            }

            let bonespace_to_bindspace = bpy_bone.matrix_local();

            joints.push(BindJoint {
                bone: bone_key,
                bindspace_to_bonespace: bonespace_to_bindspace.inverted(),
            });
        }

        let mid_to_bind = object.matrix_world().inverted();
        
        self.scene.bind_poses.insert(BindPose {
            joints,
            mid_to_bind,
        })
    }
}

impl From<SceneBuilder<'_>> for Scene {
    fn from(mut build: SceneBuilder) -> Self {
        let mut parent_links = Vec::with_capacity(build.child_oid_to_bpy_parent.len());

        dbg!(&build.bpy_parent_to_oid_parent);
        dbg!(&build.child_oid_to_bpy_parent);

        for oid in build.scene.objects.keys() {
            match build.child_oid_to_bpy_parent.get(&oid) {
                None => (),
                Some(p) => {
                    let parent_oid = build.bpy_parent_to_oid_parent[p];
                    parent_links.push((oid, parent_oid));
                },
            }
        }

        for (child, parent) in parent_links {
            build.scene.objects[child].parent = Some(parent);
            build.scene.objects[parent].children.push(child);
        }

        for (skinned, bpy_skeleton, model_to_mid) in &build.skin_requests {
            let skeleton_oid = build.bpy_parent_to_oid_parent[&BpyParent::Object(*bpy_skeleton)];
            let skeleton_obj = &build.scene.objects[skeleton_oid];
            let skele_data = match skeleton_obj.data {
                ObjectData::Armature(a) => &build.scene.bind_poses[a],
                _ => panic!("Skin reference didn't reference armature")
            };

            let joint_names = skele_data.joints.iter()
                .map(|bj| build.scene.objects[bj.bone].name.as_ref())
                .collect::<Vec<_>>();
            
            let skinned_mesh = match &build.scene.objects[*skinned].data {
                ObjectData::Mesh(me) => me,
                _ => panic!("Tried to skin a non-mesh")
            };

            let vgroup_to_joint_mapping = skinned_mesh.vertex_groups.names.iter()
                .map(|vgn| joint_names.iter().position(|jn| jn == vgn))
                .map(|i| i.unwrap())
                .collect::<Vec<_>>();

            let skinned_mesh = match &mut build.scene.objects[*skinned].data {
                ObjectData::Mesh(me) => me,
                _ => panic!("Tried to skin a non-mesh")
            };

            skinned_mesh.skin = Some(SkinReference {
                armature: skeleton_oid,
                model_to_mid: *model_to_mid,
                vgroup_to_joint_mapping,
            })
        }

        build.scene
    }
}

pub fn scene_from_bpy_selected(env: &PyEnv, data: &PyAny, meters_per_unit: f32, default_author_tag: &str) -> Scene {
    // According to the manual, it's O(len(bpy.data.objects)) to use children or children_recusive
    // so we should do a pair of iterations instead of recursing ourselves
    // specifically once over children_recursive to grab everything,
    // and once over the grabbed objects to fill in the relations.
    //
    // The actual filling in is done in <Scene as From<SceneBuilder>>::from


    let mut scene = SceneBuilder::new(env);
    scene.set_scale(meters_per_unit);

    let bpy_scene: &PyAny = get!(env.bpy_context, 'attr "scene");
    let bpy_scene_diesel: &PyAny = get!(bpy_scene, 'attr "diesel");
    let blend_data: &PyAny = get!(env.bpy_context, 'attr "blend_data");

    if bpy_scene_diesel.is_none() {
        scene.set_diesel(DieselSceneSettings {
            author_tag: default_author_tag.to_owned(),
            source_file: get!(blend_data, 'attr "filepath"),
            scene_type: "default".to_owned(),
        })
    }
    else {
        let author_tag = if get!(bpy_scene_diesel, 'attr "override_author_tag") {
            get!(bpy_scene_diesel, 'attr "author_tag")
        }
        else {
            default_author_tag.to_owned()
        };

        let source_file = if get!(bpy_scene_diesel, 'attr "override_source_path") {
            get!(bpy_scene_diesel, 'attr "source_path")
        }
        else {
            get!(blend_data, 'attr "filepath")
        };

        let scene_type = get!(bpy_scene_diesel, 'attr "scene_type");
        scene.set_diesel(DieselSceneSettings { author_tag, source_file, scene_type })
    }
    

    let data = bpy::Object::wrap(data);
    let active = scene.add_bpy_object(data);
    scene.set_active_object(active);

    for b_obj in data.iter_children_recursive() {
        scene.add_bpy_object(b_obj);
    }

    scene.into() 
}

//fn mesh_into_bpy_mesh<'py>(env: &PyEnv<'py>, name: &str, input: Mesh) -> bpy::Mesh<'py> {
//    let me: bpy::Mesh = env.bpy_data.meshes().new(name);
//    me.from_pydata()
//}