use std::collections::HashMap;

use pd2tools_rust::formats::fdm;
use pd2tools_rust::formats::fdm::DieselContainer;
use pd2tools_rust::hashindex::HashIndex;
use crate::model_ir as ir;
use crate::vek_types::*;

pub fn fdm_to_ir<'s, 'hi>(sections: &'s DieselContainer, hashlist: &'hi mut HashIndex, units_per_cm: f32, framerate: f32) -> ir::Scene {
    let scene = ir::Scene::default();
    
    let mut builder = SceneBuilder::new(sections, hashlist);

    for (id, sec) in sections.iter() {
        match sec {
            fdm::Section::ModelToolHashes(hs) => {
                for st in &(*hs).strings {
                    builder.add_to_hashlist(st.clone());
                }
            },

            fdm::Section::AuthorTag(at) => {
                println!("{:?}", at);
            }
            _ => (),
        }
    }

    for (id, sec) in sections.iter() {
        match sec {
            fdm::Section::Object3D(o) => { builder.add_object3d(id, &*o); },
            fdm::Section::Model(m) => { builder.add_model(id, m); },
            _ => ()
        }
    }

    scene
}


struct SceneBuilder<'s, 'hi> {
    fdm: &'s DieselContainer,
    hashlist: &'hi mut HashIndex,
    scene: ir::Scene,
    section_id_to_object: HashMap<u32, ir::ObjectKey>,
    parent_request: Vec<(ir::ObjectKey, u32)>,
    skin_request: Vec<(ir::ObjectKey, u32)>,
    material_mapping: HashMap<u64, ir::MaterialKey>
}
impl<'s, 'hi> SceneBuilder<'s, 'hi> {
    fn new(sections: &'s DieselContainer, hashlist: &'hi mut HashIndex) -> Self {
        Self {
            fdm: sections,
            hashlist,
            scene: ir::Scene::default(),
            section_id_to_object: HashMap::new(),
            parent_request: Vec::new(),
            skin_request: Vec::new(),
            material_mapping: HashMap::new()
        }
    }

    fn add_to_hashlist(&mut self, st: String) { self.hashlist.intern(st); }

    fn add_object3d(&mut self, sec_id: u32, ob_sec: &fdm::Object3d) -> ir::ObjectKey {
        let transform = decompose_matrix(ob_sec.transform);

        let ob = ir::Object {
            name: self.hashlist.get_hash(ob_sec.name.0).to_string(),
            parent: None,
            children: Vec::new(),
            transform,
            in_collections: Vec::new(),
            data: ir::ObjectData::None,
            skin_role: ir::SkinRole::None,
        };
        let key = self.scene.objects.insert(ob);
        self.section_id_to_object.insert(sec_id, key);
        if ob_sec.parent != 0xFFFFFFFFu32 {
            self.parent_request.push((key, ob_sec.parent));
        }
        key
    }

    fn add_model(&mut self, sec_id: u32, mo_sec: &fdm::Model) {
        let ob_key = self.add_object3d(sec_id, &mo_sec.object);
        let mesh = match &mo_sec.data {
            fdm::ModelData::BoundsOnly(bounds) => self.add_bounding_cube(&bounds),
            fdm::ModelData::Mesh(mesh) => {
                if mesh.skinbones != 0xFFFFFFFFu32 {
                    self.skin_request.push((ob_key, mesh.skinbones));
                }
                self.add_mesh(&mesh)
            },
        };

        self.scene.objects[ob_key].data = ir::ObjectData::Mesh(mesh)
    }

    fn add_mesh(&mut self, mesh: &fdm::MeshModel) -> ir::Mesh {
        let pt_gp = self.fdm.get_as::<fdm::PassthroughGP>(mesh.geometry_provider).unwrap();
        let topo_ip = self.fdm.get_as::<fdm::TopologyIP>(mesh.topology_ip).unwrap();
        let geom = self.fdm.get_as::<fdm::Geometry>(pt_gp.geometry).unwrap();
        let topo = self.fdm.get_as::<fdm::Topology>(topo_ip.topology).unwrap();

        //let me = ir::Mesh::from_indexed_tris(&geom.position, &topo.faces);
        let mut me = ir::Mesh::default();
        me.vertices = geom.position.clone();
        
        let weight_zipper = WeightZipper {
            curr: 0,
            idx_0: &geom.blend_indices_0,
            idx_1: &geom.blend_indices_1,
            weight_0: &geom.blend_weight_0,
            weight_1: &geom.blend_weight_1
        };

        for w in weight_zipper {
            me.vertex_groups.push(w.iter()
                .filter(|i| i.weight > 0.0)
                .map(|i| i.clone())
            )
        }

        if geom.color_0.len() > 0 {
            let float_colors: Vec<vek::Rgba<f32>> = geom.color_0.iter()
                .map(|i| i.map(|i| i.into()))
                .collect();
            me.vertex_colors.insert("COLOR_0".into(), float_colors);
        }

        if geom.color_1.len() > 0 {
            let float_colors: Vec<vek::Rgba<f32>> = geom.color_1.iter()
                .map(|i| i.map(|i| i.into()))
                .collect();
            me.vertex_colors.insert("COLOR_1".into(), float_colors);
        }

        let geom_texcoord = [
            geom.tex_coord_0.as_slice(),
            geom.tex_coord_1.as_slice(),
            geom.tex_coord_2.as_slice(),
            geom.tex_coord_3.as_slice(),
            geom.tex_coord_4.as_slice(),
            geom.tex_coord_5.as_slice(),
            geom.tex_coord_6.as_slice(),
            geom.tex_coord_7.as_slice(),
        ];
        let mut me_texcoord: [Vec<Vec2f>; 8] = Default::default();
        for i in 0..8 {
            if geom_texcoord[i].len() > 0 {
                me_texcoord[i].reserve(geom_texcoord[i].len());
            }
        }

        let topo_faces: &[[u16; 3]] = bytemuck::cast_slice(&topo.faces);
        for ra in &mesh.render_atoms {
            let idx_start = ra.base_index as usize;
            let idx_end = ra.base_index as usize + ra.triangle_count as usize;
            for tri in &topo_faces[idx_start..idx_end] {
                let next_fl = me.faceloops.len();
                me.faceloops.push(ir::Faceloop { vertex: tri[0] as usize, edge: 0 });
                me.faceloops.push(ir::Faceloop { vertex: tri[1] as usize, edge: 0 });
                me.faceloops.push(ir::Faceloop { vertex: tri[2] as usize, edge: 0 });

                me.polygons.push(ir::Polygon { base: next_fl, count: 3, material: ra.material as usize });
            }
        }

        me.faceloop_tangents = match (geom.normal.len(), geom.binormal.len(), geom.tangent.len()) {
            (0,0,0) => ir::TangentLayer::None,
            (l,0,0) => ir::TangentLayer::Normals(Vec::with_capacity(l)),
            (l,_,_) => ir::TangentLayer::Tangents(Vec::with_capacity(l))
        };

        for (ra_idx, ra) in mesh.render_atoms.iter().enumerate() {
            let idx_start = ra.base_index as usize;
            let idx_end = ra.base_index as usize + (ra.triangle_count as usize) * 3;

            for idx in &topo.faces[idx_start..idx_end] {
                for ti in 0..8 {
                    if let Some(uv) = geom_texcoord[ti].get((*idx) as usize) {
                        me_texcoord[ti].push(*uv);
                    }
                }

                match &mut me.faceloop_tangents {
                    ir::TangentLayer::None => (),
                    ir::TangentLayer::Normals(n) => n.push(geom.normal[(*idx) as usize]),
                    ir::TangentLayer::Tangents(t) => t.push(ir::Tangent {
                        normal: geom.normal[(*idx) as usize],
                        tangent: geom.tangent[(*idx) as usize],
                        bitangent: geom.binormal[(*idx) as usize],
                    }),
                }
            }

            if let Some(skinbones) = self.fdm.get_as::<fdm::SkinBones>(mesh.skinbones) {
                let map = skinbones.bones.mapping[ra_idx].as_slice();
                for vertex_num in ra.vertex_range() {
                    let v = &mut me.vertex_groups[vertex_num];
                    for w in v.iter_mut() {
                        let g: usize = w.group.try_into().unwrap();
                        w.group = map[g].try_into().unwrap();
                    }
                }
            }
        }

        for (i,uvl) in me_texcoord.into_iter().enumerate() {
            if uvl.len() > 0 {
                me.faceloop_uvs.insert(format!("TEXCOORD_{}", i), uvl);
            }
        }

        if let Some(mat_group) = self.fdm.get_as::<fdm::MaterialGroup>(mesh.material_group) {
            for mat_id in &mat_group.material_ids {
                me.material_ids.push(self.intern_material(*mat_id))
            }
        }
        
        me.deduplicate_vertices();
        me
    }

    fn add_bounding_cube(&mut self, bounds: &fdm::Bounds) -> ir::Mesh {
        let fdm::Bounds {min,max,..} = bounds;
        ir::Mesh {
            vertices: vec! [
                Vec3f::from((min.x,min.y,min.z)),
                Vec3f::from((min.x,min.y,max.z)),
                Vec3f::from((min.x,max.y,min.z)),
                Vec3f::from((min.x,max.y,max.z)),
                Vec3f::from((max.z,min.y,min.z)),
                Vec3f::from((max.z,min.y,max.z)),
                Vec3f::from((max.z,max.y,min.z)),
                Vec3f::from((max.z,max.y,max.z)),
            ],
            edges: vec! [
                (0, 1), (1, 3),
                (0, 2), (2, 3),

                (4, 5), (5, 7),
                (4, 6), (6, 7),

                (0, 4), (1, 5), (2, 6), (3, 7)
            ], 
            diesel: ir::DieselMeshSettings {
                cast_shadows: false,
                receive_shadows: false,
                bounds_only: true,
            },
            ..Default::default()
        }
    }

    fn intern_material(&mut self, mat_id: u32) -> Option<ir::MaterialKey> {
        let fdm_mat = self.fdm.get_as::<fdm::Material>(mat_id)?;
        match self.material_mapping.entry(fdm_mat.name) {
            std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
            std::collections::hash_map::Entry::Vacant(v) => {
                let n = self.hashlist.get_hash(fdm_mat.name);
                if n.text == Some("Material: Default Material") { return None; }
                let k = self.scene.materials.insert(ir::Material {
                    name: n.to_string(),
                });
                v.insert(k);
                Some(k)
            },
        }
    }

    fn connect_parents(&mut self) {
        let parentings = self.parent_request.iter()
            .map(|(child, parent_id)| (*child, self.section_id_to_object[parent_id]));

        for (child, parent) in parentings {
            self.scene.objects[child].parent = Some(parent);
            self.scene.objects[parent].children.push(child);
        }
    }

    fn build_skins(&mut self) {
        // I (KT) don't know if SkinBones.root_bone_object always points to something that can
        // be made into an Armature object in Blender land, so if it *is* weighted to,
        // the parent gets made into the armature.
        //
        // On top of this, any object which has a bone child is turned to bone, unless it's an
        // armature or the root (which becomes an armature).
        //
        // We're assuming that all the skins in one file have the same bind pose, too.

        struct Skin {
            armature: ir::ObjectKey,
            global_transform: Mat4f,
            joints: Vec<(ir::ObjectKey, Mat4f)>
        }

        let mut indie_skins = Vec::<(ir::ObjectKey, Skin)>::with_capacity(self.skin_request.len());

        for (skinned_object_key, skinbones_id) in &self.skin_request {
            let skinbones = self.fdm.get_as::<fdm::SkinBones>(*skinbones_id).unwrap();

            let joints = skinbones.joints.iter().map(|(bone_idx, tf)| {
                let bone_key = self.section_id_to_object[bone_idx];
                (bone_key, tf.clone())
            }).collect();

            let skin = Skin {
                armature: self.section_id_to_object[&skinbones.root_bone_object],
                global_transform: skinbones.global_skin_transform,
                joints
            };

            indie_skins.push((*skinned_object_key, skin));
        }

        
    }
}
impl<'s,'hi> From<SceneBuilder<'s,'hi>> for ir::Scene {
    fn from(value: SceneBuilder) -> Self {
        todo!()
    }
}

struct WeightZipper<'a> {
    curr: usize,
    idx_0: &'a [vek::Vec4<u16>],
    idx_1: &'a [vek::Vec4<u16>],
    weight_0: &'a [Vec4f],
    weight_1: &'a [Vec4f],
}
impl<'a> Iterator for WeightZipper<'a> {
    type Item = [ir::Weight; 8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr >= self.idx_0.len() && self.curr >= self.idx_1.len() {
            return None
        }

        let mut res = [ir::Weight::default(); 8];
        let mut c = 0;
        if self.curr < self.idx_0.len() {
            for i in 0..4 {
                res[c] = ir::Weight {
                    weight: self.weight_0[self.curr][i],
                    group: self.idx_0[self.curr][i] as usize
                };
                c += 1;
            }
        }
        if self.curr < self.idx_1.len() {
            for i in 0..4 {
                res[c] = ir::Weight {
                    weight: self.weight_1[self.curr][i],
                    group: self.idx_1[self.curr][i] as usize
                };
                c += 1;
            }
        }
        Some(res)
    }
    
}

/// Split matrix into loc/rot/scale the way Blender does.
fn decompose_matrix(mat: vek::Mat4<f32>) -> vek::Transform<f32,f32,f32>
{

    let rs: vek::Mat3<f32> = mat.into();

    let position = mat.cols[3].xyz();

    let (r0, sx) = rs.cols[0].normalized_and_get_magnitude();
    let (r1, sy) = rs.cols[1].normalized_and_get_magnitude();
    let (r2, sz) = rs.cols[2].normalized_and_get_magnitude();

    let mut rot = vek::Mat3 {
        cols: vek::Vec3 {
            x: r0,
            y: r1,
            z: r2
        }
    };
    let mut scale = vek::Vec3 { x: sx, y: sy, z: sz };

    if rot.determinant() < 0.0 {
        rot *= -1.0;
        scale *= -1.0;
    }

    let orientation = quaternion_from_mat3(rot);

    vek::Transform {
        position,
        orientation,
        scale,
    }
}

/// Convert 3x3 matrix to quaternion, even more the way blender does.
fn quaternion_from_mat3(mat: vek::Mat3<f32>) -> vek::Quaternion<f32> {
    let mut q = vek::Quaternion::zero();

    let trace = 1.0 + mat.trace();
    let mut s = 2.0 * f32::sqrt(trace);
    if mat.cols[2][2] < 0.0 {

        if mat.cols[0][0] > mat.cols[1][1] {
            if mat.cols[1][2] < mat.cols[2][1] {
                s = -s;
            }
            q.y = 0.25*s;
            s = 1.0/s;
            q.x = (mat.cols[1][2] - mat.cols[2][1]) * s;
            q.z = (mat.cols[0][1] - mat.cols[1][0]) * s;
            q.w = (mat.cols[2][0] - mat.cols[0][2]) * s;
            if trace == 1.0 && q.x == 0.0 && q.z == 0.0 && q.w == 0.0 {
                q.y = 1.0;
            }
        }
        else {
            if mat.cols[2][0] < mat.cols[0][2] {
                s = -s;
            }
            q.z = 0.25*s;
            s = 1.0/s;
            q.x = (mat.cols[2][0] - mat.cols[0][2]) * s;
            q.y = (mat.cols[0][1] - mat.cols[1][0]) * s;
            q.w = (mat.cols[1][2] - mat.cols[2][1]) * s;
            if trace == 1.0 && q.x == 0.0 && q.y == 0.0 && q.w == 0.0 {
                q.z = 1.0;
            }
        }
    }
    else {
        if mat.cols[0][0] < -mat.cols[1][1] {
            if mat.cols[0][1] < mat.cols[1][0] {
                s = -s;
            }
            q.w = 0.25*s;
            s = 1.0/s;
            q.x = (mat.cols[0][1] - mat.cols[1][0]) * s;
            q.y = (mat.cols[2][0] - mat.cols[0][2]) * s;
            q.z = (mat.cols[1][2] - mat.cols[2][1]) * s;
            if trace == 1.0 && q.x == 0.0 && q.y == 0.0 && q.z == 0.0 {
                q.w = 1.0;
            }
        }
        else {
            q.x = 0.25*s;
            s = 1.0/s;
            q.y = (mat.cols[1][2] - mat.cols[2][1]) * s;
            q.z = (mat.cols[2][0] - mat.cols[0][2]) * s;
            q.w = (mat.cols[0][1] - mat.cols[1][0]) * s;
            if trace == 1.0 && q.y == 0.0 && q.z == 0.0 && q.w == 0.0 {
                q.x = 1.0;
            }
        }
    }

    q
}