use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use bytemuck_derive::Zeroable;
use slotmap::SlotMap;

use crate::vek_types::*;

slotmap::new_key_type! {
    pub struct ObjectKey;
    pub struct MeshKey;
    pub struct LightKey;
    pub struct CameraKey;
    pub struct MaterialKey;
    pub struct CollectionKey;
    pub struct BindPoseKey;
}

#[derive(Default)]
pub struct Scene {
    pub objects: SlotMap<ObjectKey, Object>,
    pub materials: SlotMap<MaterialKey, Material>,
    pub collections: SlotMap<CollectionKey, Collection>,
    pub bind_poses: SlotMap<BindPoseKey, BindPose>,

    pub active_object: Option<ObjectKey>,
    pub meters_per_unit: f32,
    pub diesel: DieselSceneSettings
}

#[derive(Default)]
pub struct DieselSceneSettings {
    pub author_tag: String,
    pub source_file: String,
    pub scene_type: String
}

pub struct Material {
    pub name: String,
}

pub struct Collection {
    pub name: String,
    pub parent: CollectionKey,
    pub children: Vec<CollectionKey>,
    pub members: Vec<ObjectKey>
}

pub struct Object {
    pub name: String,
    pub parent: Option<ObjectKey>,
    pub children: Vec<ObjectKey>,
    pub transform: Transform,
    pub in_collections: Vec<CollectionKey>,
    pub data: ObjectData,
    pub skin_role: SkinRole,
}

#[derive(PartialEq, Eq)]
pub enum SkinRole {
    None,
    Armature,
    Bone
}

pub enum ObjectData {
    None,
    Mesh(Mesh),
    Light(Light),
    Camera(Camera),
    Armature(BindPoseKey)
}

pub struct Light;
pub struct Camera;

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub edges: Vec<(usize, usize)>,
    pub faceloops: Vec<Faceloop>,
    pub polygons: Vec<Polygon>,
    pub triangles: Vec<Triangle>,

    pub vertex_groups: VertexGroups,
    pub vertex_colors: BTreeMap<String, Vec<Rgbaf>>,
    
    pub faceloop_tangents: TangentLayer,
    pub faceloop_colors: BTreeMap<String, Vec<Rgbaf>>,
    pub faceloop_uvs: BTreeMap<String, Vec<Vec2f>>,

    pub material_names: Vec<Option<Rc<str>>>,
    pub material_ids: Vec<Option<MaterialKey>>,

    pub skin: Option<SkinReference>,
    pub diesel: DieselMeshSettings
}

impl Mesh {
    pub fn compute_local_bounds(&self) -> vek::Aabb<f32> {
        let mut vit = self.vertices.iter();
        let mut init_aabb = match vit.next() {
            Some(v) => vek::Aabb::new_empty(*v),
            None => vek::Aabb::default(),
        };
        vit.fold(init_aabb, |c,v| { c.expanded_to_contain_point(*v)} )
    }

    pub fn vcols_to_faceloop_cols(&mut self) {
        let vertex_color_attrs = std::mem::take(&mut self.vertex_colors);
        
        for (name, vcols) in vertex_color_attrs {
            let flcols = self.faceloops.iter()
                .map(|fl| vcols[fl.vertex])
                .collect();
            self.faceloop_colors.insert(name, flcols);
        }
    }

    pub fn deduplicate_vertices(&mut self) {
        self.vertex_groups.sort_weights();
        
        let mut old_to_new = Vec::<usize>::with_capacity(self.vertices.len());
        let mut new_to_old = Vec::<usize>::with_capacity(self.vertices.len());
        let mut seen_vertices = HashMap::<VertexRef,usize>::with_capacity(self.vertices.len());
        for i in 0..self.vertices.len() {
            match seen_vertices.entry(VertexRef{mesh: self, vtx: i}) {
                std::collections::hash_map::Entry::Occupied(o) => {
                    old_to_new.push(*o.get())
                },
                std::collections::hash_map::Entry::Vacant(v) => {
                    let newidx = new_to_old.len();
                    
                    old_to_new.push(newidx);
                    new_to_old.push(i);
                    v.insert(newidx);
                    
                },
            }
        }

        fn gather_attribute<T: Copy>(indices: &[usize], input: &[T]) -> Vec<T> {
            indices.iter().map(|i| input[*i]).collect()
        }

        let new_coords = gather_attribute(&new_to_old, &self.vertices);
        let new_vcol = self.vertex_colors.iter()
            .map(|(k,v)| (k.clone(),gather_attribute(&new_to_old, &v)))
            .collect();
        let mut new_vgroups = VertexGroups::default();
        for i in new_to_old.iter() {
            new_vgroups.push(self.vertex_groups[*i].iter().map(|i| i.clone()));
        }

        self.vertices = new_coords;
        self.vertex_colors = new_vcol;
        self.vertex_groups = new_vgroups;

        for i in self.edges.iter_mut() {
            i.0 = old_to_new[i.0];
            i.1 = old_to_new[i.1];
        }

        for i in self.faceloops.iter_mut() {
            i.vertex = old_to_new[i.vertex];
        }

        for i in self.edges.iter_mut() {
            i.0 = old_to_new[i.0];
            i.1 = old_to_new[i.1];
        }
        
        if self.edges.len() > 0 {
            let mut seen_edges = HashMap::with_capacity(self.edges.len());
            let mut old_to_new = Vec::<usize>::with_capacity(self.edges.len());
            let mut new_to_old = Vec::<usize>::with_capacity(self.edges.len());

            for i in 0..(self.edges.len()) {
                let cand = self.edges[i];
                let cand = if cand.1 < cand.0 { (cand.1, cand.0) } else { (cand.0, cand.1) };

                match seen_edges.entry(cand) {
                    std::collections::hash_map::Entry::Occupied(o) => old_to_new.push(*o.get()),
                    std::collections::hash_map::Entry::Vacant(v) => {
                        let newidx = new_to_old.len();
                        old_to_new.push(newidx);
                        new_to_old.push(i);
                        v.insert(newidx);
                    },
                }
            }
        }
    }
}

struct VertexRef<'m> {
    mesh: &'m Mesh,
    vtx: usize,
}
impl<'m> std::hash::Hash for VertexRef<'m> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        bytemuck::cast::<_,[u8;24]>(self.mesh.vertices[self.vtx]).hash(state);
        self.mesh.vertex_groups[self.vtx].hash(state);
        for vcol in self.mesh.vertex_colors.values() {
            bytemuck::cast::<_,[u8;32]>(vcol[self.vtx]).hash(state);
        }
    }
}
impl<'m> PartialEq for VertexRef<'m> {
    fn eq(&self, other: &Self) -> bool {
        let co_l = bytemuck::cast::<_,[u8;24]>(self.mesh.vertices[self.vtx]);
        let co_r = bytemuck::cast::<_,[u8;24]>(self.mesh.vertices[other.vtx]);
        if co_l != co_r { return false; }

        self.mesh.vertex_groups[self.vtx] == self.mesh.vertex_groups[other.vtx]
    }
}
impl<'m> Eq for VertexRef<'m> {
    fn assert_receiver_is_total_eq(&self) {}
}

pub struct DieselMeshSettings {
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub bounds_only: bool
}
impl Default for DieselMeshSettings {
    fn default() -> Self {
        Self {
            cast_shadows: true,
            receive_shadows: true,
            bounds_only: false
        }
    }
}

pub struct Faceloop {
    pub vertex: usize,
    pub edge: usize
}

#[derive(Default, Zeroable, Clone, Copy, PartialEq)]
pub struct Weight {
    pub group: usize,
    pub weight: f32
}
impl std::hash::Hash for Weight {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.group.hash(state);
        self.weight.to_bits().hash(state);
    }
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

#[derive(Default)]
pub enum TangentLayer {
    #[default] None,
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
impl std::ops::IndexMut<usize> for VertexGroups {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.vertices.as_slice().get(index)
            .map(|bc| {
                let r: std::ops::Range<usize> = bc.into();
                &mut self.weights[r]
            })
            .unwrap()
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
    pub fn push(&mut self, groups: impl Iterator<Item=Weight>) {
        let base = self.weights.len();
        self.weights.extend(groups);
        let count = self.weights.len() - base;
        self.vertices.push(BaseCount { base, count })
    }

    pub fn iter_vertex_weights(&self) -> impl Iterator<Item=(usize, &[Weight])> {
        (0..self.vertices.len())
        .map(move |i| (i, &self[i]))
    }

    pub fn sort_weights(&mut self) {
        for i in 0..(self.vertices.len()) {
            self[i].sort_by(|a,b| a.weight.partial_cmp(&b.weight).unwrap());
        }
    }
}

pub struct SkinReference {
    pub armature: ObjectKey,
    /// For each vgroup number, the index in the BindPose of the corresponding bone.
    pub vgroup_to_joint_mapping: Vec<usize>,
    /// Half of the model-to-bind transform, specifically the half that's the mesh's 
    /// model-to-world transform (if the data came from blender)
    pub model_to_mid: Mat4f
}

/// Stores the bind pose of an armature.
pub struct BindPose {
    pub joints: Vec<BindJoint>,
    /// This is half of the model-to-bind transform, specifically the half that's the inverse
    /// of the armature's model-to-world transform (if the data came from blender).
    pub mid_to_bind: Mat4f
}

pub struct BindJoint {
    pub bone: ObjectKey,
    pub bindspace_to_bonespace: Mat4f
}

impl Scene {
    /// Actually resize everything in the scene to match `new_scale`, then set that as the scale.
    /// 
    /// For [`vek::Transform`] as the transform representation, and for a uniform scale factor,
    /// this works without any care for ordering: uniform scales commute with themselves, and with 
    /// rotations. You can also pseudo-commute a scale and a translation by applying or unapplying 
    /// the scale to the translation. This allows pushing the scale all the way to individual Mesh
    /// data, which of course can be scaled.
    /// 
    /// And so you wind up with, all we have to do is scale each position.
    pub fn change_scale(&mut self, new_scale: f32) {
        let scale_factor = self.meters_per_unit / new_scale;
        
        for obj in self.objects.values_mut() {
            obj.transform.position *= scale_factor;
            match &mut obj.data {
                ObjectData::None => (),
                ObjectData::Mesh(m) => {
                    for i in m.vertices.iter_mut() {
                        *i *= scale_factor
                    }
                    
                    match &mut m.skin {
                        Some(sk) => {
                            sk.model_to_mid.cols[3].x *= scale_factor;
                            sk.model_to_mid.cols[3].y *= scale_factor;
                            sk.model_to_mid.cols[3].z *= scale_factor;
                        },
                        _ => ()
                    }
                },
                ObjectData::Light(_) => todo!(),
                ObjectData::Camera(_) => todo!(),
                ObjectData::Armature(bpk) => {
                    let bind_pose = &mut self.bind_poses[*bpk];
                    for j in bind_pose.joints.iter_mut() {
                        j.bindspace_to_bonespace.cols[3].x *= scale_factor;
                        j.bindspace_to_bonespace.cols[3].y *= scale_factor;
                        j.bindspace_to_bonespace.cols[3].z *= scale_factor;
                    }
                    bind_pose.mid_to_bind.cols[3].x *= scale_factor;
                    bind_pose.mid_to_bind.cols[3].y *= scale_factor;
                    bind_pose.mid_to_bind.cols[3].z *= scale_factor;
                },
            }
        }
        self.meters_per_unit = new_scale
    }
}

pub struct SkinRequest<I> {
    pub armature: I,
    pub global_transform: Mat4f,
    pub joints: Vec<(I, Mat4f)>
}

#[derive(Default)]
pub struct CoreBuilder<OID,MID> {
    scene: Scene,
    id_to_object: HashMap<OID, ObjectKey>,
    parent_request: Vec<(ObjectKey, OID)>,
    skin_request: Vec<(OID, SkinRequest<OID>)>,
    material_mapping: HashMap<MID, MaterialKey>
}
impl<OID, MID> std::ops::Deref for CoreBuilder<OID, MID> {
    type Target = Scene;

    fn deref(&self) -> &Self::Target {
        &self.scene
    }
}
impl<OID, MID> std::ops::DerefMut for CoreBuilder<OID, MID> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scene
    }
}

impl<OID, MID> CoreBuilder<OID, MID>
where
    OID: PartialEq + Eq + std::hash::Hash
{
    pub fn add_object(&mut self, source_id: OID, parent_id: Option<OID>, obj: Object) -> ObjectKey {
        let key = self.scene.objects.insert(obj);
        self.id_to_object.insert(source_id, key);
        if let Some(parent_id) = parent_id {
            self.parent_request.push((key, parent_id))
        }
        key
    }
    pub fn add_skin_request(&mut self, object: OID, skin_request: SkinRequest<OID>) {
        self.skin_request.push((object, skin_request))
    }
    
    pub fn build(mut self) -> Scene {
        for (child, parent) in self.parent_request {
            let parent = self.id_to_object[&parent];
            self.scene.objects[child].parent = Some(parent);
            self.scene.objects[parent].children.push(child);
        }

        let skin_requests = self.skin_request.iter().map(|(oid, sr)| SkinRequest::<ObjectKey> {
            armature: self.id_to_object[&oid],
            global_transform: sr.global_transform,
            joints: sr.joints.iter().map(|(ji,jt)| (self.id_to_object[&ji], *jt)).collect()
        }).collect::<Vec<_>>();

        /* 
        Currently we assume that skinnings don't overlap in a way that either
        - causes the SkinRequest.armature to be a bone
        - uses an object as an armature in one skin and a bone in another
        - requires two different bind poses for the same bone.
        We also assume that the armature is actually specified.

        For now we just:
        - Mark as an armature everything requested as such
        - Mark as a bone everything requested as such
        - And the ancestors of anything requested as such, up to the armature.
        - Generate a bind pose by taking the last joint matrix seen for each bone
        - Generate vgroup->joint mappings
         */ 

        let mut bone_poses: HashMap<ObjectKey, (Mat4f, Mat4f)> = Default::default();

        for sr in &skin_requests {
            let arma_obj = &mut self.scene.objects[sr.armature];
            if arma_obj.skin_role == SkinRole::Bone {
                todo!("Deal with overlapping armatures")
            }
            else {
                arma_obj.skin_role = SkinRole::Armature
            }

            for (bone_key, bone_tf) in &sr.joints {
                let mut curr_ancestor = *bone_key;
                loop {
                    let ancestor_obj = &mut self.scene.objects[curr_ancestor];
                    if ancestor_obj.skin_role != SkinRole::Armature {
                        ancestor_obj.skin_role = SkinRole::Bone
                    }
                    if let Some(a) = ancestor_obj.parent {
                        curr_ancestor = a;
                    }
                    else {
                        break
                    }
                }
            }
        }

        self.scene
    }
}