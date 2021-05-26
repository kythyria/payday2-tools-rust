///! Quite blender-like mesh representation.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use nom::IResult;

use pd2tools_macros::Parse;
use pd2tools_rust::util::parse_helpers;

#[pyclass]
#[derive(Clone, Default)]
pub struct Mesh {
    #[pyo3(get, set)] pub vertices: Vec<Vertex>,
    #[pyo3(get, set)] pub loops: Vec<Loop>,
    #[pyo3(get, set)] pub faces: Vec<Face>,

    #[pyo3(get, set)] pub material_names: Vec<Option<String>>,
    #[pyo3(get, set)] pub uv_layers: Vec<UvLayer>,
    #[pyo3(get, set)] pub colours: Vec<ColourLayer>,

    #[pyo3(get, set)] pub has_normals: bool
}
#[pymethods]
impl Mesh {

    fn position_tuples(&self, py: Python) -> PyObject {
        let position_iter = self.vertices.iter()
            .map(|i| -> Py<PyTuple> { i.co.into_py(py)});
        let position_list = PyList::new(py, position_iter);
        position_list.into()
    }

    /// Get a list of triangles, in the form of tuples of vertex indices.
    fn triangle_vertices(&self, py: Python) -> PyObject {
        let vert_iter = self.faces.iter()
            .map(|f| f.loops.iter().map(|lp| self.loops[*lp].vertex))
            .map(|vi| PyTuple::new(py, vi));
        let tv_list = PyList::new(py, vert_iter);
        tv_list.into()
    }

    fn faceloop_normals(&self, py: Python) -> PyObject {
        let loop_iter = self.loops.iter()
            .map(|lo| PyTuple::new(py, [lo.normal.0, lo.normal.1, lo.normal.2].iter()) );
        let fln_list = PyList::new(py, loop_iter);
        fln_list.into()
    }
}

#[pyclass]
#[derive(Clone, Parse)]
pub struct Vertex {
    #[pyo3(get, set)] pub co: (f32, f32, f32),
    #[pyo3(get, set)] pub weights: Vec<VertexWeight>
}

#[pyclass]
#[derive(Clone, Parse)]
pub struct VertexWeight {
    #[pyo3(get, set)] pub group: i32,
    #[pyo3(get, set)] pub weight: f32
}

#[pyclass]
#[derive(Clone)]
pub struct Loop {
    #[pyo3(get, set)] pub vertex: usize,
    #[pyo3(get, set)] pub normal: (f32, f32, f32)
}

#[pyclass]
#[derive(Clone)]
pub struct Face {
    #[pyo3(get, set)] pub material: u16,
    #[pyo3(get, set)] pub loops: Vec<usize>
}

#[pyclass]
#[derive(Clone)]
pub struct UvLayer {
    #[pyo3(get, set)] pub name: String,
    #[pyo3(get, set)] pub data: Vec<(f32, f32)>
}

#[pyclass]
#[derive(Clone)]
pub struct ColourLayer {
    #[pyo3(get, set)] pub name: String,
    #[pyo3(get, set)] pub data: Vec<(f32, f32, f32, f32)>
}