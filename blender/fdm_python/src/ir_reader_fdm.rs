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
    parent_request: Vec<(ir::ObjectKey, u32)>
}
impl<'s, 'hi> SceneBuilder<'s, 'hi> {
    fn new(sections: &'s DieselContainer, hashlist: &'hi mut HashIndex) -> Self {
        Self {
            fdm: sections,
            hashlist,
            scene: ir::Scene::default(),
            section_id_to_object: HashMap::new(),
            parent_request: Vec::new(),
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
            fdm::ModelData::Mesh(mesh) => self.add_mesh(&mesh),
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

        for (c,w) in weight_zipper {
            me.vertex_groups.push(w[0..c].iter().map(|i| i.clone()))
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

        for ra in &mesh.render_atoms {
            let idx_start = ra.base_index as usize;
            let idx_end = ra.base_index as usize + (ra.triangle_count as usize) * 3;

            for idx in &topo.faces[idx_start..idx_end] {
                for ti in 0..8 {
                    if let Some(uv) = geom_texcoord[ti].get((*idx) as usize) {
                        me_texcoord[ti].push(*uv);
                    }
                }
            }
        }

        me
    }

    fn add_bounding_cube(&mut self, bounds: &fdm::Bounds) -> ir::Mesh { todo!() }
}

struct WeightZipper<'a> {
    curr: usize,
    idx_0: &'a [vek::Vec4<u16>],
    idx_1: &'a [vek::Vec4<u16>],
    weight_0: &'a [vek::Vec4<f32>],
    weight_1: &'a [vek::Vec4<f32>],
}
impl<'a> Iterator for WeightZipper<'a> {
    type Item = (usize, [ir::Weight; 8]);

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
        res.sort_by(|a,b| a.weight.partial_cmp(&b.weight).unwrap());
        Some((c, res))
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