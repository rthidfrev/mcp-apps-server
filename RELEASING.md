# Versioning and release preparation

Release policy for `mcp-apps-server` and `mcp-apps-server-macros`.

## Compatibility

During the first alpha series, advance the prerelease suffix (`alpha.1`, `alpha.2`,
and so on), describe incompatible changes when delivering a candidate, and require
consumers to opt into the chosen alpha explicitly. The API is still being evaluated;
the alpha label does not promise compatibility between candidates. Keep both package
versions and the runtime's exact macro dependency aligned.

After the alpha series, compatible changes within `0.y` advance the patch number;
breaking changes advance the minor number. From 1.0, use semantic versioning:
breaking changes advance major, compatible additions minor, and corrections patch.

Compatibility covers public signatures, types and trait bounds, documented behavior,
protocol serialization and metadata preservation, features and defaults, exposed
dependency types and the minimum compiler version. Raising that compiler minimum is
a breaking change under this project's policy. Use
[Cargo's SemVer guidance](https://doc.rust-lang.org/cargo/reference/semver.html)
to assess the actual effect on consumers. Record published changes and migration
instructions in release notes.

## Prepare a candidate

Run these steps from the repository root.

1. Review the intended consumer contract and update versions, Cargo.lock,
   CHANGELOG.md, API docs and examples together. Update the README's alpha status and tested host versions
   when they change.
2. Run formatting, strict Clippy, relevant tests and documentation examples, and
   strict rustdoc as specified in AGENTS.md. Check distinct feature combinations
   and the declared minimum compiler; an all-features build alone is insufficient.
3. Inspect `cargo package --locked --workspace --list`, then verify both archives
   with `cargo package --locked --workspace --all-features --target-dir target/release-candidate-UNIQUE`.
   Replace `UNIQUE` with a new identifier for each candidate. Cargo uses a temporary
   registry for the workspace's unpublished macro dependency. Inspect the resulting
   `.crate` files under that directory's `package/`: include source, documentation
   and license; exclude local files and build outputs. Verify that the macro archive
   hash, temporary registry index and runtime archive's lockfile agree.
4. Inspect and commit the exact intended diff. Repackage the clean commit in a
   fresh target directory and inspect the archives' `.cargo_vcs_info.json` identity.
   Use `--allow-dirty` only for package checks before committing.

## Publication

Publish `mcp-apps-server-macros` first, then `mcp-apps-server`: the runtime package
depends on the exact macro version being available in the registry.
