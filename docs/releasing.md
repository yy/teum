# Release process

1. Confirm the working tree is clean and update `CHANGELOG.md` with the release
   date and version.
2. Set the same version in `Cargo.toml` and run:

   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features --locked -- -D warnings
   cargo test --all-targets --locked
   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --locked
   cargo audit
   cargo publish --dry-run --locked
   ```

3. Commit the release, create an annotated `vX.Y.Z` tag on that commit, and push
   both the commit and tag.
4. Create the GitHub release from that tag using the matching changelog entry.
5. Publish the immutable crate only after verifying the tag and packaged files:

   ```bash
   cargo package --list --locked
   cargo publish --locked
   ```

6. Confirm the crates.io page, docs.rs build, and clean install with
   `cargo install teum`.

Never place a crates.io token in the repository or command history. Prefer a
short-lived, least-privilege token or crates.io trusted publishing when
available for the repository.
