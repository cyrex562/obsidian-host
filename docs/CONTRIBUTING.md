# Contributing to Obsidian Host

We welcome contributions! Here's how to get started.

## Development Setup

1.  **Prerequisites**:
    -   Rust (latest stable)
    -   Node.js & npm (for frontend)
    -   SQLite (optional, bundled with SQLx)

2.  **Environment**:
    -   Copy `config.toml` example.
    -   Set `RUST_LOG=info` or `debug`.

3.  **Running Locally**:
    -   **Terminal 1 (Frontend)**:
        ```bash
        cd frontend
        npm install
        npm run watch
        ```
    -   **Terminal 2 (Backend)**:
        ```bash
        cargo run
        ```
    -   Access at http://localhost:8080.

## Project Structure
-   `src/`: Rust backend.
-   `frontend/`: TypeScript frontend.
-   `tests/`: Integration tests.

## Coding Standards
-   **Rust**: Follow `clippy` and `rustfmt`.
    -   Run `cargo fmt` before committing.
    -   Run `cargo clippy` to check for issues.
-   **TypeScript**: Use strict types.

## Testing
-   Run unit tests: `cargo test`
-   Run integration tests: `cargo test --test "*"`
-   Run specific test: `cargo test --test performance_tests`

## Pull Request Process
1.  Fork the repository.
2.  Create a feature branch.
3.  Add tests for your feature.
4.  Ensure all tests pass.
5.  Submit PR.
