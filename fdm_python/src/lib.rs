mod meshoid;
mod fdm_to_meshoid;
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

    #[pyfn(m, "get_meshoids_for_filename")]
    fn get_meshoids_for_filename(s: &str) -> PyResult<Vec<meshoid::Mesh>> {
        let bytes = match std::fs::read(s) {
            Err(e) => {
                return PyResult::Err(PyErr::from(e))
            },
            Ok(b) => b
        };

        let sections = match fdm::parse_file(&bytes) {
            Err(_) => return PyResult::Err(pyo3::exceptions::PyException::new_err("Failed parsing FDM container")),
            Ok((_, s)) => s
        };

        let mut result = Vec::<meshoid::Mesh>::new();
        let secs = sections.iter()
            .filter_map(|(_, i)| match i {
                fdm::Section::Model(m) => Some(m),
                _ => None
            });
        for sec in secs {
            let md = match sec.data {
                fdm::ModelData::Mesh(ref m) => m,
                _ => continue
            };

            let r = fdm_to_meshoid::meshoid_from_mesh(&sections, &md);
            match r {
                Ok(mo) => result.push(mo),
                Err(e) => {
                    let mut es = String::new();
                    pd2tools_rust::util::write_error_chain(&mut es, e).unwrap();
                    return PyResult::Err(pyo3::exceptions::PyException::new_err(es));
                }
            };
        }
        Ok(result)
    }

    #[pyfn(m, "import_ir_from_file")]
    fn import_ir_from_file(py: Python, hashlist_path: &str, model_path: &str) -> PyResult<Vec<Py<py_ir::Object>>> {
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

        let r = ir_reader::sections_to_ir(py, &sections, &hashlist);
        r.map_err(|e| {
            let mut es = String::new();
            pd2tools_rust::util::write_error_chain(&mut es, e).unwrap();
            pyo3::exceptions::PyException::new_err(es)
        })
    }

    Ok(())
    
}