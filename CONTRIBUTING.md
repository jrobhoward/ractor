# Contributing to this library

We want to make contributing to this project as easy and transparent as
possible.

## Be respectful

Duh.

## Workspace Structure

This repository is a Cargo workspace with multiple crates:

- **ractor**: Core actor framework (default member)
- **ractor_cluster**: Distributed cluster support (default member)
- **ractor_cluster_derive**: Procedural macros (default member)
- **ractor_shell**: Interactive debugging REPL (**optional**, not built by default)

### Building ractor_shell

The `ractor_shell` crate is excluded from default workspace builds to keep the core library lightweight. To work with it:

```bash
# Build ractor_shell specifically
cargo build -p ractor_shell

# Run ractor_shell tests
cargo test -p ractor_shell

# Run ractor_shell examples
cargo run --example demo -p ractor_shell
```

## Pull Requests

We actively welcome your pull requests!

1. Fork the repo and create your branch from `main`.
2. If you've added code that should be tested, add tests.
3. If you've changed APIs, update the documentation.
4. Ensure the test suite passes.

## Issues

We use GitHub issues to track issues.

For reported bugs, please ensure your description is clear and has sufficient instructions to be able to reproduce the issue.

For feature requests, please try to be detailed in what you'd like to see so we can address it properly!

## License

By contributing to `ractor`, you agree that your contributions will be
licensed under the LICENSE file in the root directory of this source tree.
