use std::collections::HashMap;

use pd2tools_rust::formats::fdm;
use pd2tools_rust::formats::fdm::DieselContainer;
use pd2tools_rust::hashindex::HashIndex;
use crate::model_ir as ir;

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
        let passthrough_gp = match &self.fdm[mesh.geometry_provider] {
            fdm::Section::PassthroughGP(pgp) => &*pgp,
            _ => panic!("GeoIP didn't point to a PassthroughGP")
        };

        let topo_ip = match &self.fdm[mesh.topology_ip] {
            fdm::Section::TopologyIP(tip) => &*tip,
            _ => panic!("TopoIP didn't point to a TopoIP")
        };
        
        ir::Mesh {
            vertices: todo!(),
            edges: todo!(),
            faceloops: todo!(),
            polygons: todo!(),
            triangles: todo!(),
            tangents: todo!(),
            vertex_groups: todo!(),
            vertex_colors: todo!(),
            faceloop_colors: todo!(),
            faceloop_uvs: todo!(),
            material_names: todo!(),
            material_ids: todo!(),
            skin: todo!(),
            diesel: todo!(),
        }
    }

    fn add_bounding_cube(&mut self, bounds: &fdm::Bounds) -> ir::Mesh { todo!() }
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