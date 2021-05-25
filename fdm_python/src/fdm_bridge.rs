use std::convert::TryInto;

type Vec3f = vek::Vec3<f32>;

use pd2tools_rust::formats::fdm;
use crate::meshoid;

pub fn meshoid_from_geometry(geo: &fdm::GeometrySection, topo: &fdm::TopologySection, atoms: &[fdm::RenderAtom]) -> meshoid::Mesh {
    let mut mesh = meshoid::Mesh::default();

    mesh.vertices.reserve(geo.position.len());
    for i in 0..geo.position.len() {
        mesh.vertices.push(meshoid::Vertex {
            weights: Vec::with_capacity((geo.weightcount_0 + geo.weightcount_1) as usize),
            co: geo.position[i].into_tuple()
        })
    }

    for ra in atoms {
        mesh.material_names.push(format!("mat_{}", ra.material));

        for i in (ra.base_index)..(ra.base_index + ra.triangle_count) {
            let v0_ri = topo.faces[i as usize].0 as usize;
            let v1_ri = topo.faces[i as usize].1 as usize;
            let v2_ri = topo.faces[i as usize].2 as usize;

            let v0_i = v0_ri + ra.base_vertex as usize;
            let v1_i = v1_ri + ra.base_vertex as usize;
            let v2_i = v2_ri + ra.base_vertex as usize;

            mesh.edges.push(indexes_to_edge( v0_i, v1_i ));
            mesh.edges.push(indexes_to_edge( v1_i, v2_i ));
            mesh.edges.push(indexes_to_edge( v2_i, v0_i ));
            
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 3,
                vertex: v0_ri
            });
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 2,
                vertex: v1_ri
            });
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 1,
                vertex: v2_ri
            });

            mesh.faces.push(meshoid::Face {
                loops: vec![
                    mesh.loops.len() - 3,
                    mesh.loops.len() - 2,
                    mesh.loops.len() - 1
                ],
                material: ra.material.try_into().unwrap()
            })
        }
    }

    return mesh;
}

fn vec3f_to_vertex(v: Vec3f) -> meshoid::Vertex {
    meshoid::Vertex {
        weights: Vec::new(),
        co: (v.x, v.y, v.z)
    }
}

fn indexes_to_edge(s: usize, e: usize) -> meshoid::Edge {
    meshoid::Edge {
        seam: false,
        sharp: false,
        vertices: ( s, e )
    }
}