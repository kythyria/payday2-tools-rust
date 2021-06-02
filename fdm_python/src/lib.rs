mod py_ir;
mod ir_reader;

use pyo3::prelude::*;

use pd2tools_rust::formats::fdm;

#[pymodule]
fn pd2tools_fdm(_py: Python, m: &PyModule) -> PyResult<()> {

    #[pyfn(m, "diesel_hash")]
    fn diesel_hash(s: &str) -> u64 {
        pd2tools_rust::diesel_hash::from_str(s)
    }

    #[pyfn(m, "import_ir_from_file")]
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

        let sections = match fdm::parse_file(&bytes) {
            Err(_) => return PyResult::Err(pyo3::exceptions::PyException::new_err("Failed parsing FDM container")),
            Ok((_, s)) => s
        };

        let r = ir_reader::sections_to_ir(py, &sections, &hashlist, units_per_cm, framerate);
        r.map_err(|e| {
            let mut es = String::new();
            pd2tools_rust::util::write_error_chain(&mut es, e).unwrap();
            pyo3::exceptions::PyException::new_err(es)
        })
    }

    Ok(())
    
}