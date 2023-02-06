use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use pyo3::types::PyDict;
use pyo3::{prelude::*, intern, AsPyPointer};
use crate::{ PyEnv, model_ir, bpy_binding };
use model_ir::*;

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Transform = vek::Transform<f32, f32, f32>;
type Quaternion = vek::Quaternion<f32>;

type PyObjPtr = *mut pyo3::ffi::PyObject;

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

fn mat3_from_bpy_matrix(bmat: &PyAny) -> vek::Mat3<f32> {
    let mut floats = [[0f32; 3]; 3];
    for c in 0..3 {
        let col = bmat.get_item(c).unwrap();
        for r in 0..3 {
            let cell = col.get_item(r).unwrap().extract::<f32>().unwrap();
            floats[c][r] = cell;
        }
    }
    vek::Mat3::from_col_arrays(floats)
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

fn quaternion_from_bpy_quat(e: &PyEnv, bq: &PyAny) -> Quaternion {
    let x: f32 = get!(e, bq, 'attr "x");
    let y: f32 = get!(e, bq, 'attr "y");
    let z: f32 = get!(e, bq, 'attr "z");
    let w: f32 = get!(e, bq, 'attr "w");
    Quaternion::from_xyzw(x, y, z, w)
}

fn transform_from_bpy_matrix(env: &PyEnv, bmat: &PyAny) -> Transform {
    let py_lrs = bmat.call_method0(intern!{env.python, "decompose"}).unwrap();
    let (py_loc, py_rot, py_scale): (&PyAny, &PyAny, &PyAny) = py_lrs.extract().unwrap();
    Transform {
        position: vek3f_from_bpy_vec(env, py_loc),
        orientation: quaternion_from_bpy_quat(env, py_rot),
        scale: vek3f_from_bpy_vec(env, py_scale)
    }
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

fn mesh_from_bpy_mesh(env: &PyEnv, data: &PyAny) -> model_ir::Mesh {
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

    let mut vertex_colors = BTreeMap::new();
    let mut faceloop_colors = BTreeMap::new();
    for att in get!(env, data, 'iter "attributes") {
        let data_type = get!(env, att, 'attr "data_type");
        let data = match data_type {
            "FLOAT_COLOR" => get!(env, att, 'iter "data")
                .map(|i| from_bpy_array(get!(env, i, 'attr "color")))
                .collect::<Vec<Vec4f>>(),
            "BYTE_COLOR" => get!(env, att, 'iter "data")
                .map(|i| from_bpy_array(get!(env, i, 'attr "color")))
                .collect::<Vec<Vec4f>>(),
            _ => continue
        };

        let name: String = get!(env, att, 'attr "name");

        match get!(env, att, 'attr "domain") {
            "POINT" => vertex_colors.insert(name, data),
            "CORNER" => faceloop_colors.insert(name, data),
            _ => panic!("Implausible vcol domain")
        };
    }

    let faceloop_uvs = get!(env, data, 'iter "uv_layers")
        .map(|uvl| {
            let name: String = get!(env, uvl, 'attr "name");
            let uvs: Vec<Vec2f> = get!(env, uvl, 'iter "data")
                .map(|uv| vek2f_from_bpy_vec(env, get!(env, uv, 'attr "uv")))
                .collect();
            (name, uvs)
        })
        .collect::<BTreeMap<_,_>>();

    let tangents = if faceloop_uvs.is_empty() { // and thus tangent data will be invalid
        TangentLayer::Normals(get!(env, data, 'iter "loops")
            .map(|lp| vek3f_from_bpy_vec(env, get!(env, lp, 'attr "normal")))
            .collect()
        )
    }
    else { // We have tangents!
        TangentLayer::Tangents(get!(env, data, 'iter "loops")
            .map(|lp| Tangent {
                normal: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "normal")),
                tangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "tangent")),
                bitangent: vek3f_from_bpy_vec(env, get!(env, lp, 'attr "bitangent"))
            })
            .collect()
        )
    };

    let mut diesel = DieselMeshSettings::default();
    let bpy_diesel: &PyAny = get!(env, data, 'attr "diesel");
    if !bpy_diesel.is_none() {
        diesel.cast_shadows = get!(env, bpy_diesel, 'attr "cast_shadows");
        diesel.receive_shadows = get!(env, bpy_diesel, 'attr "receive_shadows");
        diesel.bounds_only = get!(env, bpy_diesel, 'attr "bounds_only");
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

/// Wrapper for an evaluated, triangulated mesh that frees it automatically
/// 
/// Presumably io_scene_gltf does this sort of thing because otherwise the temp meshes would
/// pile up dangerously fast? IDK, but it does it so we're cargo culting.
struct TemporaryMesh<'py> {
    mesh: &'py PyAny,
    object: &'py PyAny,
    python: Python<'py>
}
impl<'py> TemporaryMesh<'py> {
    /// Get the evaluated mesh for an object
    fn from_depgraph(env: &'py PyEnv, object: &'py PyAny) -> TemporaryMesh<'py> {
        let depsgraph = env.b_c_evaluated_depsgraph_get().unwrap();
        let evaluated_obj = object.call_method1(intern!{env.python, "evaluated_get"}, (depsgraph,))
            .unwrap();

        let to_mesh_args = PyDict::new(env.python);
        to_mesh_args.set_item("preserve_all_data_layers", true).unwrap();
        to_mesh_args.set_item("depsgraph", depsgraph).unwrap();
        let mesh = evaluated_obj.call_method(intern!{env.python, "to_mesh"}, (), Some(to_mesh_args))
            .unwrap();

        if mesh.getattr(intern!(env.python, "uv_layers")).unwrap().len().unwrap() > 0 {
            // Calculate the tangents here, because this can fail if the mesh still has ngons,
            // and this is where we make a new mesh anyway
            match mesh.call_method0(intern!{env.python, "calc_tangents"}) {
                Ok(_) => (),
                Err(_) => {
                    let bm = bpy_binding::bmesh::new(env.python).unwrap();
                    bm.from_mesh(mesh).unwrap();
                    let faces = bm.faces().unwrap();
                    env.bmesh_ops.triangulate(&bm, faces).unwrap();
                    bm.to_mesh(mesh).unwrap();
                    mesh.call_method0(intern!{env.python, "calc_tangents"}).unwrap();
                },
            }
        }
        
        TemporaryMesh {
            mesh,
            object: evaluated_obj,
            python: env.python
        }
    }
}
impl Drop for TemporaryMesh<'_> {
    fn drop(&mut self) {
        match self.object.call_method0(intern!{self.python, "to_mesh_clear"}) {
            _ => () // the worst that can happen is we leak memory, I think?
        }
    }
}
impl std::ops::Deref for TemporaryMesh<'_> {
    type Target = PyAny;

    fn deref(&self) -> &Self::Target {
        self.mesh
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
    bpy_armature_to_skin: HashMap<PyObjPtr, SkinKey>
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
            bpy_armature_to_skin: HashMap::new()
        }
    }

    fn set_scale(&mut self, meters_per_unit: f32) { self.scene.meters_per_unit = meters_per_unit }
    fn set_active_object(&mut self, active_object: ObjectKey) {
        self.scene.active_object = Some(active_object)
    }
    fn set_diesel(&mut self, di: DieselSceneSettings) {
        self.scene.diesel = di;
    }
    
    fn add_bpy_object(&mut self, object: &PyAny) -> ObjectKey {
        // If this is an armature, we have to worry about bone-parented and skinned children.
        // Children whose parent_type is BONE are parented to a bone.
        // Children whose parent type is OBJECT but have an Armature Deform modifier are skinned.
        // Children whose parent_type is ARMATURE just act like that.

        let otype: &str = get!(self.env, object, 'attr "type");

        let odata = match otype {
            "MESH" => ObjectData::Mesh(self.add_bpy_mesh_instance(object)),
            "EMPTY" => ObjectData::None,
            "ARMATURE" => ObjectData::Armature(self.add_bpy_armature_bones(object)),
            _ => todo!()
        };

        let new_obj = Object {
            name: get!(self.env, object, 'attr "name"),
            parent: None,
            children: Vec::new(),
            transform: transform_from_bpy_matrix(self.env, get!(self.env, object, 'attr "matrix_local")),
            in_collections: Vec::new(),
            data: odata,
            skin_role: if otype == "ARMATURE" { SkinRole::Armature } else { SkinRole::None }
        };
        let oid = self.scene.objects.insert(new_obj);

        self.bpy_parent_to_oid_parent.insert(BpyParent::Object(object.as_ptr()), oid);
        let parent = object.getattr(intern!{self.env.python, "parent"}).unwrap();
        if !parent.is_none() {
            let parent_type: &str = get!(self.env, object, 'attr "parent_type");
            let pkey = match parent_type {
                "OBJECT" => BpyParent::Object(parent.as_ptr()),
                "BONE" => {
                    let bone_name: String = get!(self.env, object, 'attr "parent_bone");
                    if bone_name.is_empty() {
                        BpyParent::Object(parent.as_ptr())
                    }
                    else {
                        BpyParent::Bone(parent.as_ptr(), bone_name)
                    }
                },
                "ARMATURE" => todo!("Skinning"),
                _ => panic!("Unknown parent type {}", parent_type)
            };
            self.child_oid_to_bpy_parent.insert(oid, pkey);
        }

        oid
    }

    fn add_bpy_mesh_instance(&mut self, object: &PyAny) -> Mesh {
        let data = TemporaryMesh::from_depgraph(self.env, object);
        let mut mesh = mesh_from_bpy_mesh(self.env, &data);

        mesh.vertex_groups.names = get!(self.env, object, 'iter "vertex_groups")
            .map(|vg| get!(self.env, vg, 'attr "name"))
            .collect();

        let mats = get!(self.env, object, 'iter "material_slots")
            .map(|ms| get!(self.env, ms, 'attr "material"))
            .collect::<Vec<&PyAny>>();

        mesh.material_names.clear();
        mesh.material_names.extend(
            mats.iter()
            .map(|mat| {
                if mat.is_none() { return None }
                let st: String = get!(self.env, mat, 'attr "name");
                Some(Rc::from(st))
            })
        );

        mesh.material_ids.clear();
        mesh.material_ids.extend(
            mats.iter()
            .map(|mat| {
                if mat.is_none() { return None }
                Some(self.add_bpy_material(mat))
            })
        );

        mesh
    }

    fn add_bpy_material(&mut self, mat: &PyAny) -> MaterialKey {
        if self.bpy_mat_to_matid.contains_key(&mat.as_ptr()) {
            return self.bpy_mat_to_matid[&mat.as_ptr()]
        }

        let new_mat = Material {
            name: get!(self.env, mat, 'attr "name"),
        };

        self.scene.materials.insert(new_mat)
    }

    fn add_bpy_armature_bones(&mut self, object: &PyAny) -> SkinKey {
        let mut joints = Vec::new();

        let data: &PyAny = get!(self.env, object, 'attr "data");
        for bpy_bone in get!(self.env, data, 'iter "bones") {
            // For some reason things being parented to bone tails *isn't* a display trick.
            // Bones really are stored that way.
            // So the position of a bone is its head position plus the parent's tail pos.
            // And the rotation comes from the `matrix` property.
            let bone_name: String = get!(self.env, bpy_bone, 'attr "name");
            let head = vek3f_from_bpy_vec(self.env, get!(self.env, bpy_bone, 'attr "head"));
            let rot_mat: &PyAny = get!(self.env, bpy_bone, 'attr "matrix");
            let rot = rot_mat.call_method0(intern!{self.env.python, "to_quaternion"}).unwrap();
            let rot = quaternion_from_bpy_quat(self.env, rot);
            let parent: &PyAny = get!(self.env, bpy_bone, 'attr "parent");
            let parent_tail = if parent.is_none() { Vec3f::new(0.0, 0.0, 0.0) }
            else {
                vek3f_from_bpy_vec(self.env, get!(self.env, parent, 'attr "tail"))
            };

            let transform = Transform {
                position: parent_tail + head,
                orientation: rot,
                scale: vek::Vec3::one()
            };

            let bone_obj = Object {
                name: bone_name.clone(),
                parent: None,
                children: Vec::new(),
                transform,
                in_collections: Vec::new(),
                data: ObjectData::None,
                skin_role: SkinRole::Bone,
            };

            let bone_key = self.scene.objects.insert(bone_obj);
            self.bpy_parent_to_oid_parent
                .insert(BpyParent::Bone(object.as_ptr(), bone_name.clone()), bone_key);
            if parent.is_none() {
                self.child_oid_to_bpy_parent
                .insert(bone_key, BpyParent::Object(object.as_ptr()));
            }
            else {
                let parent_name = get!(self.env, parent, 'attr "name");
                self.child_oid_to_bpy_parent.insert(bone_key, BpyParent::Bone(object.as_ptr(), parent_name));
            }

            let bonespace_to_bindspace = get!(self.env, bpy_bone, 'attr "matrix_local");
            let bonespace_to_bindspace = mat4_from_bpy_matrix(bonespace_to_bindspace);

            joints.push(SkinJoint {
                bone: bone_key,
                bindspace_to_bonespace: bonespace_to_bindspace.inverted(),
            });
        }
        
        self.scene.skins.insert(Skin {
            joints,
            world_to_bind: vek::Mat4::identity(),
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

    let bpy_scene: &PyAny = get!(env, env.bpy_context, 'attr "scene");
    let bpy_scene_diesel: &PyAny = get!(env, bpy_scene, 'attr "diesel");
    let blend_data: &PyAny = get!(env, env.bpy_context, 'attr "blend_data");

    if bpy_scene_diesel.is_none() {
        scene.set_diesel(DieselSceneSettings {
            author_tag: default_author_tag.to_owned(),
            source_file: get!(env, blend_data, 'attr "filepath"),
            scene_type: "default".to_owned(),
        })
    }
    else {
        let author_tag = if get!(env, bpy_scene_diesel, 'attr "override_author_tag") {
            get!(env, bpy_scene_diesel, 'attr "author_tag")
        }
        else {
            default_author_tag.to_owned()
        };

        let source_file = if get!(env, bpy_scene_diesel, 'attr "override_source_path") {
            get!(env, bpy_scene_diesel, 'attr "source_path")
        }
        else {
            get!(env, blend_data, 'attr "filepath")
        };

        let scene_type = get!(env, bpy_scene_diesel, 'attr "scene_type");
        scene.set_diesel(DieselSceneSettings { author_tag, source_file, scene_type })
    }
    

    let active = scene.add_bpy_object(data);
    scene.set_active_object(active);

    for b_obj in get!(env, data, 'iter "children_recursive") {
        scene.add_bpy_object(b_obj);
    }

    scene.into() 
}