use pyo3::prelude::*;

fn main() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        println!("Hello {}", py.version());
    });
}
