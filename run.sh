#!/bin/bash
# Helper script to run Obsidian Host with correct Python environment

# Ensure we are using the local python version defined by .python-version
# (This requires pyenv to be in the path)

if ! command -v pyenv &> /dev/null; then
    echo "Error: pyenv not found. Please install pyenv."
    exit 1
fi

# Get the path to the python executable and library directory
PYTHON_BIN=$(pyenv which python)
PYTHON_PREFIX=$(pyenv prefix)

echo "Using Python: $PYTHON_BIN"

# Export environment variables for pyo3
export PYO3_PYTHON="$PYTHON_BIN"
export LD_LIBRARY_PATH="$PYTHON_PREFIX/lib:$LD_LIBRARY_PATH"

# Run the server
cargo run
