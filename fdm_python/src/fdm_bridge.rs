use std::convert::TryInto;

type Vec3f = vek::Vec3<f32>;
use pyo3::number::index;

use pd2tools_rust::formats::fdm;
use crate::meshoid;

pub fn meshoid_from_geometry(geo: &fdm::GeometrySection, topo: &fdm::TopologySection, atoms: &[fdm::RenderAtom]) -> meshoid::Mesh {
    let mut mesh = meshoid::Mesh::default();

    for ra in atoms {
        mesh.material_names.push(format!("mat_{}", ra.material));

        for i in (ra.base_index)..(ra.base_index + ra.triangle_count) {
            let v0_i = topo.faces[i as usize].0 as usize;
            let v1_i = topo.faces[i as usize].1 as usize;
            let v2_i = topo.faces[i as usize].2 as usize;

            mesh.vertices.push(vec3f_to_vertex(geo.position[v0_i + (ra.base_vertex as usize)]));
            mesh.vertices.push(vec3f_to_vertex(geo.position[v1_i + (ra.base_vertex as usize)]));
            mesh.vertices.push(vec3f_to_vertex(geo.position[v2_i + (ra.base_vertex as usize)]));

            mesh.edges.push(indexes_to_edge( mesh.vertices.len() - 3, mesh.vertices.len() - 2 ));
            mesh.edges.push(indexes_to_edge( mesh.vertices.len() - 2, mesh.vertices.len() - 1 ));
            mesh.edges.push(indexes_to_edge( mesh.vertices.len() - 1, mesh.vertices.len() - 3 ));
            
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 3,
                vertex: mesh.vertices.len() - 3
            });
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 2,
                vertex: mesh.vertices.len() - 2
            });
            mesh.loops.push(meshoid::Loop {
                normal: (0.0, 0.0, 0.0),
                edge: mesh.edges.len() - 1,
                vertex: mesh.vertices.len() - 1
            });

            mesh.faces.push(meshoid::Face {
                loops: (mesh.loops.len() - 3, 3),
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