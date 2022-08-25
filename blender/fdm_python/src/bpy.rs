pub mod types {
    use pyo3::{PyAny, Python};

    pub struct Object<'py>(&'py PyAny);
    impl<'py> Object<'py> {
        fn name(&self) -> String {
            self.0.call_method0("name").unwrap().to_string()
        }
    }
}