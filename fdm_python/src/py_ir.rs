//! Glue representation.
//!
//! These structs are bundles of "convert chunk of data to/from python" routines,
//! and a holder for the Rust representation of that chunk. It's all Vec and Tuple
//! because this struct of arrays approach requires less fancy python code to copy
//! into Blender, in particular, `vert_positions`, `edges`, and `faces` can just be
//! passed straight to Mesh.from_pydata.
//!
//! Pyo3 will actually make the conversion routines for us, if we ask for getters
//! and setters, but then insist on executing them on every single get, which is a
//! rather substantial performance issue for meshes.

use pyo3::prelude::*;

#[pyclass]
pub struct Armature { }
#[pyclass]
pub struct Animation { }

#[pyclass]
pub struct Object {
    #[pyo3(get, set)] pub name: String,
    #[pyo3(get, set)] pub parent: Option<Py<Object>>,

    #[pyo3(get, set)] pub location: (f32, f32, f32),
    #[pyo3(get, set)] pub rotation: (f32, f32, f32, f32),
    #[pyo3(get, set)] pub scale: (f32, f32, f32),

    #[pyo3(get, set)] pub animations: Option<Py<Animation>>,
    #[pyo3(get, set)] pub data: Option<PyObject>
}

#[pyclass]
pub struct Light { }

#[pyclass]
pub struct Camera { }

#[pyclass]
#[derive(Default)]
pub struct Mesh {
    #[pyo3(get, set)] pub material_names: Vec<String>,
    #[pyo3(get, set)] pub weight_names: Vec<String>,
    #[pyo3(get, set)] pub has_normals: bool,

    #[pyo3(get, set)] pub vert_positions: Vec<(f32, f32, f32)>,
    #[pyo3(get, set)] pub vert_weights: Vec<Vec<(u32, f32)>>,

    #[pyo3(get, set)] pub edges: Vec<(usize, usize)>,
    /// (sharp, seam)
    #[pyo3(get, set)] pub edge_flags: Vec<(bool, bool)>,

    #[pyo3(get, set)] pub faces: Vec<(usize, usize, usize)>,
    #[pyo3(get, set)] pub face_materials: Vec<usize>,

    #[pyo3(get, set)] pub loop_normals: Vec<(f32, f32, f32)>,
    #[pyo3(get, set)] pub loop_uv_layers: Vec<Vec<(f32, f32)>>,
    #[pyo3(get, set)] pub loop_colour_layers: Vec<(f32, f32, f32, f32)>
}
#[pymethods]
impl Mesh {
    #[new]
    fn new() -> Self { Self::default() }

    #[getter]
    pub fn get_data_type(&self) -> &str { "MESH" }
}

#[pyclass]
#[derive(Default)]
pub struct BoundsObject {
    #[pyo3(get, set)] pub box_max: (f32, f32, f32),
    #[pyo3(get, set)] pub box_min: (f32, f32, f32)
}
#[pymethods]
impl BoundsObject {
    #[new]
    fn new() -> Self { Self::default() }

    #[getter]
    pub fn get_data_type(&self) -> &str { "BOUNDS" }
}