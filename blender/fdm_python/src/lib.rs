mod py_ir;
mod ir_reader_fdm;
mod ir_writer_oil;
mod model_ir;
mod ir_blender;

use pyo3::prelude::*;

use pd2tools_rust::formats::fdm;
use pd2tools_rust::util::LIB_VERSION;

#[pymodule]
fn pd2tools_fdm(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add("LIB_VERSION", LIB_VERSION)?;

    #[pyfunction]
    fn diesel_hash(s: &str) -> u64 {
        pd2tools_rust::diesel_hash::from_str(s)
    }

    #[pyfunction]
    fn import_ir_from_file(py: Python, hashlist_path: &str, model_path: &str, units_per_cm: f32, framerate: f32) -> PyResult<Vec<Py<py_ir::Object>>> {
        let hlp = Some(String::from(hashlist_path));
        let hashlist = pd2tools_rust::get_hashlist(&hlp);
        let hashlist = match hashlist {
            Some(h) => h,
            None => return PyResult::Err(pyo3::exceptions::PyException::new_err("Failed to load hashlist"))
        };

        let bytes = match std::fs::read(model_path) {
            Err(e) => {
                return PyResult::Err(PyErr::from(e))
            },
            Ok(b) => b
        };

        let sections = match fdm::parse_stream(&mut bytes.as_slice()) {
            Err(e) => {
                let msg = format!("Failed parsing FDM: {}", e);
                return PyResult::Err(pyo3::exceptions::PyException::new_err(msg))
            },
            Ok(s) => s
        };

        let r = ir_reader_fdm::sections_to_ir(py, &sections, &hashlist, units_per_cm, framerate);
        r.map_err(|e| {
            let mut es = String::new();
            pd2tools_rust::util::write_error_chain(&mut es, e).unwrap();
            pyo3::exceptions::PyException::new_err(es)
        })
    }

    #[pyfunction]
    fn export_oil(py: Python, output_path: &str, units_per_cm: f32, author_tag: &str, object: &PyAny) -> PyResult<()> {
        let env = PyEnv::new(py);
        ir_writer_oil::export(env, output_path, units_per_cm, author_tag, object)
    }

    m.add_function(wrap_pyfunction!(diesel_hash, m)?)?;
    m.add_function(wrap_pyfunction!(import_ir_from_file, m)?)?;
    m.add_function(wrap_pyfunction!(export_oil, m)?)?;

    Ok(())
    
}

#[derive(Clone, Copy)]
pub struct PyEnv<'py> {
    pub python: Python<'py>,
    pub bpy_context: &'py PyAny,
    pub bmesh: &'py PyModule,
    pub bmesh_ops: bpy_binding::bmesh::Ops<'py>,
    id_fn: &'py PyAny,
}

impl<'py> PyEnv<'py> {
    pub fn new(python: Python<'py>) -> PyEnv<'py> {
        let builtins = python.import("builtins").unwrap();
        PyEnv {
            python,
            id_fn: builtins.getattr("id").unwrap(),
            bpy_context: python.import("bpy")
                .unwrap()
                .getattr("context")
                .unwrap(),
            bmesh: python.import("bmesh")
                .unwrap(),
            bmesh_ops: bpy_binding::bmesh::Ops::import(python)
        }
    }
    pub fn id(&self, pyobj: &'py PyAny) -> u64 {
        self.id_fn.call1( (pyobj,) ).unwrap().extract::<u64>().unwrap()
    }

    pub fn b_c_evaluated_depsgraph_get(&self) -> PyResult<&PyAny> {
        self.bpy_context.call_method0(pyo3::intern!{self.python, "evaluated_depsgraph_get"})
    }
}

mod bpy_binding {
    pub mod bmesh {
        use pyo3::{intern, prelude::*};

        pub fn new<'py>(py: Python<'py>) -> PyResult<BMesh<'py>> {
            BMesh::new(py)
        }

        pub struct BMesh<'py>(&'py PyAny, Python<'py>);
        impl Drop for BMesh<'_> {
            fn drop(&mut self) {
                match self.0.call_method0(intern!{self.1, "free"}) {
                    _ => ()
                }
            }
        }
        impl<'py> BMesh<'py> {
            pub fn new(py: Python<'py>) -> PyResult<BMesh<'py>> {
                py.import("bmesh")
                    .unwrap()
                    .call_method0(intern!{py, "new"})
                    .map(|bm| BMesh(bm, py))
            }
            pub fn free(self) { }
            pub fn from_mesh(&self, mesh: &'py PyAny) -> PyResult<()> {
                self.0.call_method1(intern!{self.1, "from_mesh"}, (mesh,))
                    .map(|_|())
            }
            pub fn faces(&self) -> PyResult<&'py PyAny> {
                self.0.getattr(intern!{self.1, "faces"})
            }
            pub fn to_mesh(&self, mesh: &'py PyAny) -> PyResult<()> {
                self.0.call_method1(intern!{self.1, "to_mesh"}, (mesh,))
                    .map(|_|())
            }
        }
        impl IntoPy<pyo3::Py<pyo3::PyAny>> for BMesh<'_> {
            fn into_py(self, _py: Python<'_>) -> pyo3::Py<pyo3::PyAny> {
                self.0.into()
            }
        }
        impl IntoPy<pyo3::Py<pyo3::PyAny>> for &BMesh<'_> {
            fn into_py(self, _py: Python<'_>) -> pyo3::Py<pyo3::PyAny> {
                self.0.into()
            }
        }

        #[derive(Clone, Copy)]
        pub struct Ops<'py>(&'py PyModule, Python<'py>);
        impl<'py> Ops<'py> {
            pub fn import(py: Python<'py>) -> Self {
                Self(py.import("bmesh.ops").unwrap(), py)
            }
            pub fn triangulate(&self, mesh: &'py BMesh<'py>, faces: &'py PyAny) -> PyResult<&PyAny> {
                let args = pyo3::types::PyDict::new(self.1);
                args.set_item("faces", faces).unwrap();
                self.0.call_method(intern!{self.1, "triangulate"}, (mesh,), Some(args))
            }
        }
    }
}