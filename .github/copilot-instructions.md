# GitHub Copilot Instructions

You are an expert AI programming assistant working on the `acme-disk-use` project, a high-performance disk usage analyzer written in Rust.

## General Guidelines

- **Follow `CONTRIBUTING.md`**: Always adhere to the guidelines specified in [CONTRIBUTING.md](../CONTRIBUTING.md). This is your primary source of truth for development workflows and standards.
- **Rust Idioms**: Write idiomatic Rust code. Follow the official [Rust Style Guide](https://github.com/rust-lang/style-team).
- **Performance**: This is a performance-sensitive tool.
    - Use `rayon` for parallel processing where appropriate.
    - Minimize allocations and I/O operations.
    - Use `bincode` for efficient serialization/deserialization of the cache.
- **Safety**: Prefer safe Rust. Only use `unsafe` if absolutely necessary and strictly documented.

## Development Workflow

When generating code or suggesting changes, ensure they comply with the project's workflow:

1.  **Formatting**: Code must be formatted with `cargo fmt`.
2.  **Linting**: Code must pass `cargo clippy --all-targets --all-features -- -D warnings`.
3.  **Testing**:
    - Add unit tests for new logic in the same file (`#[cfg(test)] mod tests`).
    - Add integration tests in `tests/` if needed.
    - Ensure `cargo test --all-features` passes.

## Project Structure

- `src/lib.rs`: Library entry point.
- `src/main.rs`: CLI entry point.
- `src/scanner.rs`: Directory scanning logic (parallelized with `rayon`).
- `src/cache.rs`: Cache management logic (using `bincode`).
- `src/disk_use.rs`: High-level interface combining scanner and cache.
- `benches/`: Benchmarks using `criterion`.

## Commit Messages

If asked to generate commit messages, follow the Conventional Commits format as described in `CONTRIBUTING.md`:

```
<type>: <subject>

<body>

<footer>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`.
