///! Quite blender-like mesh representation.

use pyo3::prelude::*;

#[pyclass]
#[derive(Clone, Default)]
pub struct Mesh {
    #[pyo3(get, set)] pub vertices: Vec<Vertex>,
    #[pyo3(get, set)] pub edges: Vec<Edge>,
    #[pyo3(get, set)] pub loops: Vec<Loop>,
    #[pyo3(get, set)] pub faces: Vec<Face>,

    #[pyo3(get, set)] pub material_names: Vec<String>,
    #[pyo3(get, set)] pub uv_layers: Vec<UvLayer>,
    #[pyo3(get, set)] pub colours: Vec<ColourLayer>

}
#[pyclass]
#[derive(Clone)]
pub struct Vertex {
    #[pyo3(get, set)] pub co: (f32, f32, f32),
    #[pyo3(get, set)] pub weights: Vec<VertexWeight>
}

#[pyclass]
#[derive(Clone)]
pub struct VertexWeight {
    #[pyo3(get, set)] pub group: i32,
    #[pyo3(get, set)] pub weight: f32
}

#[pyclass]
#[derive(Clone)]
pub struct Edge {
    #[pyo3(get, set)] pub sharp: bool,
    #[pyo3(get, set)] pub seam: bool,
    #[pyo3(get, set)] pub vertices: (usize, usize)
}

#[pyclass]
#[derive(Clone)]
pub struct Loop {
    #[pyo3(get, set)] pub vertex: usize,
    #[pyo3(get, set)] pub edge: usize,
    #[pyo3(get, set)] pub normal: (f32, f32, f32)
}

#[pyclass]
#[derive(Clone)]
pub struct Face {
    #[pyo3(get, set)] pub material: u16,
    #[pyo3(get, set)] pub loops: (usize, usize)
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