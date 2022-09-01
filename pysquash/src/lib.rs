
use pyo3::prelude::*;




/// A Python module implemented in Rust.
#[pymodule]
fn pysquash(_py: Python, m: &PyModule) -> PyResult<()> {
    Ok(())
}
