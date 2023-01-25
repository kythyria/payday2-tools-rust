use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use slotmap::SlotMap;

type Vec2f = vek::Vec2<f32>;
type Vec3f = vek::Vec3<f32>;
type Vec4f = vek::Vec4<f32>;
type Mat4f = vek::Mat4<f32>;
type Transform = vek::Transform<f32, f32, f32>;

slotmap::new_key_type! {
    pub struct ObjectKey;
    pub struct MeshKey;
    pub struct LightKey;
    pub struct CameraKey;
    pub struct MaterialKey;
    pub struct CollectionKey;
    pub struct SkinKey;
}

#[derive(Default)]
pub struct Scene {
    pub objects: SlotMap<ObjectKey, Object>,
    pub materials: SlotMap<MaterialKey, Material>,
    pub collections: SlotMap<CollectionKey, Collection>,
    pub skins: SlotMap<SkinKey, Skin>,

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
}

pub struct Light;
pub struct Camera;

pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub edges: Vec<(usize, usize)>,
    pub faceloops: Vec<Faceloop>,
    pub polygons: Vec<Polygon>,
    pub triangles: Vec<Triangle>,

    pub tangents: TangentLayer,
    pub vertex_groups: VertexGroups,

    pub faceloop_colors: HashMap<String, Vec<Vec4f>>,
    pub faceloop_uvs: HashMap<String, Vec<Vec2f>>,

    pub material_names: Vec<Option<Rc<str>>>,
    pub material_ids: Vec<Option<MaterialKey>>,

    pub skin: Option<SkinKey>,
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

pub enum TangentLayer {
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
}

pub struct Skin {
    pub armature: ObjectKey,
    pub joints: Vec<SkinJoint>,
    pub model_to_bind: Mat4f
}

pub struct SkinJoint {
    pub bone: ObjectKey,
    pub bind_to_bone: Mat4f
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
                },
                ObjectData::Light(_) => todo!(),
                ObjectData::Camera(_) => todo!(),
            }
        }
        self.meters_per_unit = new_scale
    }
}