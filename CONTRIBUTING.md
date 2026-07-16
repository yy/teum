# Contributing to teum

Thanks for your interest in contributing!

## Getting started

1. Fork and clone the repo
2. Install Rust via [rustup](https://rustup.rs/)
3. Run `cargo build --locked` to verify everything compiles
4. Run `cargo test --all-targets --locked` to make sure tests pass

## Before submitting a PR

- `cargo fmt` — format your code
- `cargo clippy --all-targets --all-features --locked -- -D warnings` — no warnings
- `cargo test --all-targets --locked` — all tests pass
- Keep commits focused; one logical change per commit

## Reporting bugs

Open an issue with:
- What you expected
- What happened instead
- Steps to reproduce
- OS and Rust version (`rustc --version`)

## Code style

- Follow standard Rust conventions
- Add tests for new functionality
- Keep the CLI minimal — teum is intentionally simple
