use std::collections::HashMap;
use std::convert::TryInto;

type Vec2f = vek::Vec2<f32>;
type Rgba = vek::Rgba<f32>;

use pd2tools_rust::formats::fdm;
use crate::meshoid;

/// Convert geometry to a meshoid somewhat more intelligently.
///
/// Vertices with equivalent position and weights are merged.
/// Don't generate edges explicitly, we don't need them.
pub fn meshoid_from_geometry(geo: &fdm::GeometrySection, topo: &fdm::TopologySection, atoms: &[fdm::RenderAtom]) -> meshoid::Mesh {
    let vcache = merge_vertices(geo);

    let mut uvs = Vec::<(&Vec<Vec2f>, meshoid::UvLayer)>::new();
    let mut colors = Vec::<(&Vec<Rgba>, meshoid::ColourLayer)>::new();
    let mut loops = Vec::<meshoid::Loop>::new();
    let mut faces = Vec::<meshoid::Face>::new();
    let mut material_names = Vec::<Option<String>>::new();

    let has_normals = geo.normal.len() > 0;

    macro_rules! add_texcoord {
        ($f:ident, $n:literal) => {
            if geo.$f.len() > 0 {
                uvs.push((&geo.$f, meshoid::UvLayer{ name: String::from($n), data: Vec::new() }))
            }
        }
    }
    add_texcoord!(tex_coord_0, "uv_0");
    add_texcoord!(tex_coord_1, "uv_1");
    add_texcoord!(tex_coord_2, "uv_2");
    add_texcoord!(tex_coord_3, "uv_3");
    add_texcoord!(tex_coord_4, "uv_4");
    add_texcoord!(tex_coord_5, "uv_5");
    add_texcoord!(tex_coord_6, "uv_6");
    add_texcoord!(tex_coord_7, "uv_7");

    if geo.color_0.len() > 0 {
        colors.push((&geo.color_0, meshoid::ColourLayer {
            name: String::from("Col_0"),
            data: Vec::new()
        }))
    };
    if geo.color_1.len() > 0 {
        colors.push((&geo.color_1, meshoid::ColourLayer {
            name: String::from("Col_1"),
            data: Vec::new()
        }))
    };

    for ra in atoms {
        material_names.push(Some(format!("mat_{}", ra.material)));

        for i in (ra.base_index)..(ra.base_index + ra.triangle_count) {
            let v0_ri = topo.faces[i as usize].0 as usize;
            let v1_ri = topo.faces[i as usize].1 as usize;
            let v2_ri = topo.faces[i as usize].2 as usize;

            let v0_i = v0_ri + ra.base_vertex as usize;
            let v1_i = v1_ri + ra.base_vertex as usize;
            let v2_i = v2_ri + ra.base_vertex as usize;
            
            let merged_v0 = vcache.index_map[v0_i];
            let merged_v1 = vcache.index_map[v1_i];
            let merged_v2 = vcache.index_map[v2_i];
            
            loops.push(meshoid::Loop {
                vertex: merged_v0,
                normal: if has_normals { geo.normal[v0_i].into_tuple() } else { (0.0, 0.0, 0.0) }
            });

            loops.push(meshoid::Loop {
                vertex: merged_v1,
                normal: if has_normals { geo.normal[v1_i].into_tuple() } else { (0.0, 0.0, 0.0) }
            });

            loops.push(meshoid::Loop {
                vertex: merged_v2,
                normal: if has_normals { geo.normal[v2_i].into_tuple() } else { (0.0, 0.0, 0.0) }
            });

            for (src, ref mut dest) in colors.iter_mut() {
                dest.data.push(src[v0_i].into_tuple());
                dest.data.push(src[v1_i].into_tuple());
                dest.data.push(src[v2_i].into_tuple());
            }

            for (src, ref mut dest) in uvs.iter_mut() {
                dest.data.push(src[v0_i].into_tuple());
                dest.data.push(src[v1_i].into_tuple());
                dest.data.push(src[v2_i].into_tuple());
            }

            faces.push(meshoid::Face {
                material: ra.material.try_into().unwrap(),
                loops: vec![ 
                    loops.len() - 3,
                    loops.len() - 2,
                    loops.len() - 1
                 ]
            })
        }
    }

    meshoid::Mesh {
        vertices: vcache.vertices,
        loops, faces, material_names, has_normals,
        colours: colors.into_iter().map(|i| i.1).collect(),
        uv_layers: uvs.into_iter().map(|i| i.1).collect(),
    }
}

struct VertexCache {
    vertices: Vec<meshoid::Vertex>,
    /// Same size as original buffer, containing where in `vertices` the one at this index got merged to.
    index_map: Vec<usize>,
}


fn merge_vertices(geo: &fdm::GeometrySection) -> VertexCache {
    // For now we only merge bitwise-equivalent vertices.
    // This should be enough to undo automatic splitting.

    use pd2tools_rust::util::parse_helpers::*;

    let mut vertices = Vec::<meshoid::Vertex>::with_capacity(geo.position.len());
    let mut index_map = Vec::<usize>::with_capacity(geo.position.len());
    let mut value_cache = HashMap::<Vec<u8>, usize>::with_capacity(geo.position.len());

    let bufsize = 12 + 4 + 4 + 16 + 4 + 16;
    for i in 0..geo.position.len() {
        let mut vtx = meshoid::Vertex {
            co: geo.position[i].into_tuple(),
            weights: Vec::with_capacity(8)
        };
        
        for j in 0..geo.weightcount_0 {
            vtx.weights.push(meshoid::VertexWeight {
                group: geo.blend_indices_0[i][j as usize] as i32,
                weight: geo.blend_weight_0[i][j as usize]
            });
        }
        
        for j in 0..geo.weightcount_1 {
            vtx.weights.push(meshoid::VertexWeight {
                group: geo.blend_indices_1[i][j as usize] as i32,
                weight: geo.blend_weight_1[i][j as usize]
            });
        }
        
        let mut buf = Vec::<u8>::with_capacity(bufsize);
        vtx.serialize(&mut buf).unwrap();

        let entry = value_cache.entry(buf);
        match entry {
            std::collections::hash_map::Entry::Occupied(o) => index_map.push(*o.get()),
            std::collections::hash_map::Entry::Vacant(v) => {
                index_map.push(vertices.len());
                v.insert(vertices.len());
                vertices.push(vtx);
            }
        }
    }

    VertexCache {
        vertices, index_map
    }
}