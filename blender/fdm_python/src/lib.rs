mod py_ir;
mod ir_reader_fdm;
mod ir_writer_oil;
mod model_ir;
mod gather_from_blender;

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
    fn export_oil(py: Python, output_path: &str, units_per_cm: f32, framerate: f32, object: &PyAny) -> PyResult<()> {
        let env = PyEnv::new(py);
        ir_writer_oil::export(env, output_path, units_per_cm, framerate, object)
    }

    m.add_function(wrap_pyfunction!(diesel_hash, m)?)?;
    m.add_function(wrap_pyfunction!(import_ir_from_file, m)?)?;
    m.add_function(wrap_pyfunction!(export_oil, m)?)?;

    Ok(())
    
}

#[derive(Clone, Copy)]
pub struct PyEnv<'py> {
    pub python: Python<'py>,
    id_fn: &'py PyAny,
}

impl<'py> PyEnv<'py> {
    pub fn new(python: Python<'py>) -> PyEnv<'py> {
        let builtins = python.import("builtins").unwrap();
        PyEnv {
            python,
            id_fn: builtins.getattr("id").unwrap()
        }
    }
    pub fn id(&self, pyobj: &'py PyAny) -> u64 {
        self.id_fn.call1( (pyobj,) ).unwrap().extract::<u64>().unwrap()
    }
}

#[enumflags2::bitflags]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum ExportFlag {
    Normals,
    Tangents,
    TexCoords,
    Colors,
    Weights
}
type ExportFlags = enumflags2::BitFlags<ExportFlag>;