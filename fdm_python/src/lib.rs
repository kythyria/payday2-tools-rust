use pyo3::prelude::*;

#[pymodule]
fn pd2tools_fdm(_py: Python, m: &PyModule) -> PyResult<()> {

    #[pyfn(m, "diesel_hash")]
    fn diesel_hash(s: &str) -> u64 {
        pd2tools_rust::diesel_hash::from_str(s)
    }
    Ok(())
    
}