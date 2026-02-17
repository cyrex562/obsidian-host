use pyo3::prelude::*;

fn main() {
    println!("Testing PyO3...");
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        println!("Python Version: {}", py.version());
        let sys = py.import("sys").expect("Failed to import sys");
        println!("Sys module: {:?}", sys);
    });
}
