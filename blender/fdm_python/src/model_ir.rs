use std::collections::{HashMap, HashSet, BTreeMap};
use std::rc::Rc;

use bytemuck::Zeroable;
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

    /// Convert vertex position and index buffer into a mesh
    /// 
    /// Vertex indices are preserved, this just makes faceloops, tris, and polys.
    /// Currently does not understand restarts or make edges.
    pub fn from_indexed_tris(vtx_co: &[Vec3f], indices: &[u16]) -> Self {
        let mut faceloops = Vec::with_capacity(indices.len());
        let mut polygons = Vec::with_capacity(indices.len()/3);
        let mut triangles = Vec::with_capacity(indices.len()/3);
        
        let index_tris: &[[u16; 3]] = bytemuck::cast_slice(indices);
        for tri in index_tris {
            let fl0 = Faceloop { vertex: tri[0].into(), edge: 0 };
            let fl1 = Faceloop { vertex: tri[1].into(), edge: 0 };
            let fl2 = Faceloop { vertex: tri[2].into(), edge: 0 };

            let next_fl = faceloops.len();
            faceloops.push(fl0);
            faceloops.push(fl1);
            faceloops.push(fl2);

            let next_poly = polygons.len();
            polygons.push(Polygon { base: next_fl, count: 3, material: 0 });
            triangles.push(
                Triangle { loops: [next_fl+0, next_fl+1, next_fl+2], polygon: next_poly }
            );
        }

        Mesh {
            vertices: vtx_co.to_owned(),
            faceloops,
            polygons,
            triangles,
            ..Default::default()
        }
    }

    fn vertex_attrib_to_faceloop<T: Clone>(&self, attrib: &[T]) -> Vec<T> {
        self.faceloops.iter()
            .map(|fl| attrib[fl.vertex].clone())
            .collect()
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

#[derive(Default, Zeroable, Clone, Copy)]
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