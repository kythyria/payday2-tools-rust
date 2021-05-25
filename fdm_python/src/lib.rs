mod meshoid;
mod fdm_bridge;

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

            let gp = match &sections[&md.geometry_provider] {
                fdm::Section::PassthroughGP(pgp) => pgp,
                _ => return PyResult::Err(pyo3::exceptions::PyException::new_err("Mesh doesn't point at GP"))
            };
            let geo = match &sections[&gp.geometry] {
                fdm::Section::Geometry(pgp) => pgp,
                _ => return PyResult::Err(pyo3::exceptions::PyException::new_err("GP doesn't point at Geometry"))
            };
            let topo = match &sections[&gp.topology] {
                fdm::Section::Topology(pgp) => pgp,
                _ => return PyResult::Err(pyo3::exceptions::PyException::new_err("GP doesn't point at Topology"))
            };

            let r = fdm_bridge::meshoid_from_geometry(geo, topo, &md.render_atoms);
            result.push(r);
        }
        Ok(result)
    }

    Ok(())
    
}