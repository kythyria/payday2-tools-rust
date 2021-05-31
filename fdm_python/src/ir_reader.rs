use std::collections::HashMap;
use std::collections::HashSet;

use pyo3::{IntoPy, Python, Py, PyErr, PyObject};
use thiserror::Error;
type Vec2f = vek::Vec2<f32>;
type Rgba = vek::Rgba<u8>;

use pd2tools_macros::Parse;
use pd2tools_rust::hashindex::HashIndex;
use pd2tools_rust::formats::fdm;
use pd2tools_rust::util::parse_helpers::{self, Parse};
use crate::py_ir as ir;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Expected section {1} to be a {0:?}")]
    BadSectionType(fdm::SectionType, u32),

    #[error("Section {0} doesn't exist")]
    MissingSection(u32),

    #[error("Python error: {0}")]
    PythonError(#[from] PyErr),

    #[error("Object {0} has bad parent")]
    BadParent(u32, #[source] Box<ConversionError>),

    #[error("Failed to convert object {0}")]
    CouldntConvertObject(u32, #[source] Box<ConversionError>)
}
type ConvResult<T> = Result<T, ConversionError>;

trait ConvResultExt {
    type Out;
    fn at_object_id(self, oid: u32) -> Self::Out;
}
impl<T> ConvResultExt for ConvResult<T> {
    type Out = ConvResult<T>;
    fn at_object_id(self, oid: u32) -> Self::Out {
        self.map_err(|e| ConversionError::CouldntConvertObject(oid, Box::new(e)))
    }
}

pub fn sections_to_ir<'s, 'hi, 'py>(py: Python<'py>, sections: &'s HashMap<u32, fdm::Section>, hashlist: &'hi HashIndex) -> ConvResult<Vec<Py<ir::Object>>> {
    let mut reader = IrReader {
        py, sections, hashlist,
        objects: HashMap::new()
    };

    let ids = sections.iter().filter_map(|(k, v)| match v {
        fdm::Section::Object3D(_) |
        fdm::Section::Model(_) => Some(*k),
        _ => None
    }).collect::<Vec<u32>>();

    for i in ids {
        reader.get_object(i)?;
    }
    Ok(reader.objects.drain().map(|(k,v)| v).collect::<Vec<_>>())
}

macro_rules! expect_section {
    ($doc:expr, $target:expr, $want:ident) => {
        match &$doc[&$target] {
            fdm::Section::$want(s) => Ok(s),
            _ => Err(ConversionError::BadSectionType(fdm::SectionType::$want, $target))
        }
    }
}

struct IrReader<'s, 'hi, 'py> {
    py: Python<'py>,
    sections: &'s HashMap<u32, fdm::Section>,
    hashlist: &'hi HashIndex,
    objects: HashMap<u32, Py<ir::Object>>
}

impl<'s, 'hi, 'py> IrReader<'s, 'hi, 'py> {
    /// Actually import an object.
    fn import_object3d(&mut self, id: u32, obj: &fdm::Object3dSection) -> ConvResult<Py<ir::Object>> {
        let name = self.hashlist.get_hash(obj.name.0).to_string();
        let parent = match self.get_object(obj.parent) {
            Ok(p) => p,
            Err(e) => return Err(ConversionError::BadParent(id, Box::new(e)))
        };
        Ok(Py::new(self.py, ir::Object {
            name, parent,
            transform: mat_to_row_tuples(obj.transform),
            animations: None,
            data: None,
            weight_names: Vec::new()
        })?)
    }

    /// Obtain an object by it's section ID, or None if it doesn't exist at all.
    fn get_object(&mut self, id: u32) -> ConvResult<Option<Py<ir::Object>>> {
        if let Some(obj) = self.objects.get(&id) {
            return Ok(Some(obj.clone()));
        }
        if id == 0 {
            return Ok(None);
        }
        match self.sections.get(&id) {
            Some(fdm::Section::Object3D(sec)) => {
                let obj = self.import_object3d(id, sec)?;
                self.objects.insert(id, obj.clone());
                Ok(Some(obj))
            },
            Some(fdm::Section::Model(md)) => {
                let obj = self.import_model(id, md)?;
                self.objects.insert(id, obj.clone());
                Ok(Some(obj))
            }
            //Some(fdm::Section::Camera(_)) => todo!(),
            //Some(fdm::Section::Light(_)) => todo!(),
            Some(_) =>
                Err(ConversionError::BadSectionType(fdm::SectionType::Object3D, id)),
            None =>
                Err(ConversionError::MissingSection(id))
        }
    }

    fn import_model(&mut self, id: u32, md: &fdm::ModelSection) -> ConvResult<Py<ir::Object>> {
        let obj = self.import_object3d(id, &md.object).at_object_id(id)?;
        
        match &md.data {
            fdm::ModelData::BoundsOnly(bo) => self.import_bounds(id, obj.clone(), &bo),
            fdm::ModelData::Mesh(me) => self.import_mesh(id, obj.clone(), &me)
        }.at_object_id(id)?;

        Ok(obj)
    }

    fn import_bounds(&mut self, id: u32, obj: Py<ir::Object>, bounds: &fdm::Bounds) -> ConvResult<()> {
        let data: PyObject = Py::new(self.py, ir::BoundsObject {
            box_max: bounds.max.into_tuple(),
            box_min: bounds.min.into_tuple()
        })?.into_py(self.py);
        let mut objref = obj.borrow_mut(self.py);
        objref.data = Some(data);
        Ok(())
    }

    fn import_mesh(&mut self, id: u32, obj: Py<ir::Object>, src: &fdm::MeshModel) -> ConvResult<()> {
        let gp = expect_section!(self.sections, src.geometry_provider, PassthroughGP)?;
        let geo = expect_section!(self.sections, gp.geometry, Geometry)?;
        let topo = expect_section!(self.sections, gp.topology, Topology)?;
        let materials = expect_section!(self.sections, src.material_group, MaterialGroup)?;

        let mut material_names = Vec::<String>::new();
        for material_id in materials.material_ids.iter() {
            let material = expect_section!(self.sections, *material_id, Material)?;
            let hs = self.hashlist.get_hash(material.name);
            material_names.push(hs.to_string());
        }

        let vcache = merge_vertices(geo);
        let vertex_map = vcache.index_map;
        let mut mesh = ir::Mesh {
            material_names,
            has_normals: geo.normal.len() > 0,
            vert_positions: vcache.positions,
            vert_weights: vcache.weights,
            edges: Vec::with_capacity(topo.faces.len() * 3),
            edge_flags: Vec::with_capacity(topo.faces.len() * 3),
            faces: Vec::with_capacity(topo.faces.len()),
            face_materials: Vec::with_capacity(topo.faces.len()),
            loop_normals: Vec::with_capacity(topo.faces.len() * 3),
            loop_uv_layers: Vec::with_capacity(8),
            loop_colour_layers: Vec::with_capacity(2)
        };
        let mut seen_edges = HashSet::<(usize, usize)>::new();
        let mut uv_sources = Vec::<&Vec<Vec2f>>::with_capacity(8);
        let mut color_sources = Vec::<&Vec<Rgba>>::with_capacity(2);

        macro_rules! add_texcoord {
            ($f:ident, $n:literal) => {
                if geo.$f.len() > 0 {
                    mesh.loop_uv_layers.push((String::from($n), Vec::with_capacity(topo.faces.len() * 3)));
                    uv_sources.push(&geo.$f);
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
            mesh.loop_colour_layers.push((String::from("col_0"), Vec::with_capacity(topo.faces.len() * 3)));
            color_sources.push(&geo.color_0);
        }
        if geo.color_1.len() > 0 {
            mesh.loop_colour_layers.push((String::from("col_1"), Vec::with_capacity(topo.faces.len() * 3)));
            color_sources.push(&geo.color_1);
        }

        for ra in &src.render_atoms {
            let base_face = ra.base_index / 3;
            assert_eq!(base_face * 3, ra.base_index);

            for i in (base_face)..(base_face + ra.triangle_count) {
                let v0_i = topo.faces[i as usize].0 as usize;
                let v1_i = topo.faces[i as usize].1 as usize;
                let v2_i = topo.faces[i as usize].2 as usize;
                
                let m_v0 = vertex_map[v0_i];
                let m_v1 = vertex_map[v1_i];
                let m_v2 = vertex_map[v2_i];

                let e0 = ( usize::min(m_v0, m_v1), usize::max(m_v0, m_v1) );
                let e1 = ( usize::min(m_v1, m_v2), usize::max(m_v1, m_v2) );
                let e2 = ( usize::min(m_v2, m_v0), usize::max(m_v2, m_v0) );
                if seen_edges.insert(e0) {
                    mesh.edges.push(e0); mesh.edge_flags.push((false, false));
                }
                if seen_edges.insert(e1) {
                    mesh.edges.push(e1); mesh.edge_flags.push((false, false));
                }
                if seen_edges.insert(e2) {
                    mesh.edges.push(e2); mesh.edge_flags.push((false, false));
                }

                for i in 0..color_sources.len() {
                    mesh.loop_colour_layers[i].1.push(rgba_bytes_to_float(color_sources[i][v0_i]));
                    mesh.loop_colour_layers[i].1.push(rgba_bytes_to_float(color_sources[i][v1_i]));
                    mesh.loop_colour_layers[i].1.push(rgba_bytes_to_float(color_sources[i][v2_i]));
                }
                
                for i in 0..uv_sources.len() {
                    mesh.loop_uv_layers[i].1.push(uv_sources[i][v0_i].into_tuple());
                    mesh.loop_uv_layers[i].1.push(uv_sources[i][v1_i].into_tuple());
                    mesh.loop_uv_layers[i].1.push(uv_sources[i][v2_i].into_tuple());
                }

                if mesh.has_normals {
                    mesh.loop_normals.push(geo.normal[v0_i].into_tuple());
                    mesh.loop_normals.push(geo.normal[v1_i].into_tuple());
                    mesh.loop_normals.push(geo.normal[v2_i].into_tuple());
                }

                mesh.face_materials.push(ra.material as usize);
                mesh.faces.push((m_v0, m_v1, m_v2));
            }
        }

        let mut objref = obj.borrow_mut(self.py);
        let data = Py::new(self.py, mesh)?;
        objref.data = Some(data.into_py(self.py));

        Ok(())
    }
}

struct VertexCache {
    positions: Vec<(f32, f32, f32)>,
    weights: Vec<Vec<(u32, f32)>>,
    /// Same size as original buffer, containing where in `vertices` the one at this index got merged to.
    index_map: Vec<usize>,
}

#[derive(Clone, Parse)]
struct VertexKey {
    co: (f32, f32, f32),
    weights: Vec<(u32, f32)>
}

fn merge_vertices(geo: &fdm::GeometrySection) -> VertexCache {
    // For now we only merge bitwise-equivalent vertices.
    // This should be enough to undo automatic splitting.

    let mut positions = Vec::<(f32, f32, f32)>::with_capacity(geo.position.len());
    let mut weights = Vec::<Vec<(u32, f32)>>::with_capacity(geo.position.len());
    let mut index_map = Vec::<usize>::with_capacity(geo.position.len());
    let mut value_cache = HashMap::<Vec<u8>, usize>::with_capacity(geo.position.len());

    let bufsize = 12 + 4 + 4 + 16 + 4 + 16;
    for i in 0..geo.position.len() {
        let mut vtx = VertexKey {
            co: geo.position[i].into_tuple(),
            weights: Vec::with_capacity(8)
        };
        
        for j in 0..geo.weightcount_0 {
            vtx.weights.push((
                geo.blend_indices_0[i][j as usize] as u32,
                geo.blend_weight_0[i][j as usize]
            ));
        }
        
        for j in 0..geo.weightcount_1 {
            vtx.weights.push((
                geo.blend_indices_1[i][j as usize] as u32,
                geo.blend_weight_1[i][j as usize]
            ));
        }
        
        let mut buf = Vec::<u8>::with_capacity(bufsize);
        vtx.serialize(&mut buf).unwrap();

        let entry = value_cache.entry(buf);
        match entry {
            std::collections::hash_map::Entry::Occupied(o) => index_map.push(*o.get()),
            std::collections::hash_map::Entry::Vacant(v) => {
                index_map.push(positions.len());
                v.insert(positions.len());
                positions.push(vtx.co);
                weights.push(vtx.weights);
            }
        }
    }

    VertexCache {
        positions, index_map, weights
    }
}

fn mat_to_row_tuples(src: vek::Mat4<f32>) ->(
    (f32, f32, f32, f32),
    (f32, f32, f32, f32),
    (f32, f32, f32, f32),
    (f32, f32, f32, f32)
)
{
    let rows = src.into_row_arrays();
    (
        (rows[0][0], rows[0][1],rows[0][2],rows[0][3]),
        (rows[1][0], rows[1][1],rows[1][2],rows[1][3]),
        (rows[2][0], rows[2][1],rows[2][2],rows[2][3]),
        (rows[3][0], rows[3][1],rows[3][2],rows[3][3])
    )
}

fn rgba_bytes_to_float(c: Rgba) -> (f32, f32, f32, f32) {
    (
        (c.r as f32)/255.0,
        (c.g as f32)/255.0,
        (c.b as f32)/255.0,
        (c.a as f32)/255.0
    )
}