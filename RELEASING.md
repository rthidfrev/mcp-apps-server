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
   CHANGELOG.md, API docs and examples together. Update the README's alpha status
   and tested host versions when they change.
2. Run formatting, strict Clippy, relevant tests and documentation examples, and
   strict rustdoc as specified in AGENTS.md. Check distinct feature combinations
   and the declared minimum compiler; an all-features build alone is insufficient.
3. Inspect `cargo package --locked --workspace --list`, then verify both archives
   with `cargo package --locked --workspace --all-features --target-dir target/release-candidate-UNIQUE`.
   Replace `UNIQUE` with a new identifier for each candidate. Cargo uses a temporary
   registry for dependencies between the packaged workspace crates. Inspect the
   resulting `.crate` files under that directory's `package/`: include source, documentation
   and license; exclude local files and build outputs. Verify that the macro archive
   hash, temporary registry index and runtime archive's lockfile agree.
4. Inspect and commit the exact intended diff. Repackage the clean commit in a
   fresh target directory and inspect the archives' `.cargo_vcs_info.json` identity.
   Use `--allow-dirty` only for package checks before committing.

## Publication

Use this procedure for a new version after preparing and pushing a clean release
commit and checking its GitHub CI results. Both package versions and the exact
macro dependency must agree, and the new versions must not already be published.
Documentation-only updates can be committed to GitHub and included in the next
release; they do not require republishing an existing version.

Run the commands below in **PowerShell at the repository root**, one at a time.
Stop on an error before running the next command. Keep the same clean checkout
throughout both publications. `cargo publish` uploads the package to crates.io;
an uploaded version cannot be overwritten.

### 1. Authenticate and identify the release

For a first publication, sign in to [crates.io](https://crates.io), verify your
email in the [account settings](https://crates.io/settings/profile), and create an
[API token](https://crates.io/settings/tokens). Enable `publish-new` for new crates
and `publish-update` for subsequent versions. If Cargo does not already have a
valid token, run:

```powershell
cargo login --registry crates-io
```

Paste the token at Cargo's prompt. Cargo stores it locally for authentication;
keep it out of repository files and command-line arguments.

Read the prepared version from Cargo's workspace metadata, record its commit,
and choose a fresh directory for publication checks:

```powershell
$releaseVersion = (cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json).packages |
    Where-Object { $_.name -eq "mcp-apps-server" } |
    Select-Object -ExpandProperty version
$releaseCommit = git rev-parse HEAD
$publishTarget = "target/publish-" + [guid]::NewGuid().ToString("N")
git status --short --branch
```

The status must show no changed or untracked files. Each command below uses a new
subdirectory so earlier package archives cannot be mistaken for its output.
`--locked` preserves dependency versions from Cargo.lock.

### 2. Verify and publish the macros

Publish `mcp-apps-server-macros` first: the runtime depends on its exact version
being available in the registry. First prepare and rebuild the archive without
uploading it:

```powershell
cargo publish -p mcp-apps-server-macros --registry crates-io --locked --dry-run --target-dir "$publishTarget/macros-check"
```

The warning `aborting upload due to dry run` is expected. If verification succeeds,
publish the macros:

```powershell
cargo publish -p mcp-apps-server-macros --registry crates-io --locked --target-dir "$publishTarget/macros-publish"
```

Wait for Cargo's `Published` confirmation before proceeding to the runtime. Cargo
waits for the uploaded version to appear in the registry index. If that wait times
out after uploading, check the exact version on
[crates.io](https://crates.io/crates/mcp-apps-server-macros) before retrying; the
upload may already have succeeded.

### 3. Verify and publish the runtime

Once the macro version is available, verify the runtime archive with all optional
features, including the server adapter and macros:

```powershell
cargo publish -p mcp-apps-server --registry crates-io --locked --all-features --dry-run --target-dir "$publishTarget/runtime-check"
```

If verification succeeds, publish the runtime:

```powershell
cargo publish -p mcp-apps-server --registry crates-io --locked --all-features --target-dir "$publishTarget/runtime-publish"
```

Wait for its `Published` confirmation. On an upload or index-wait error, check
the exact version on [crates.io](https://crates.io/crates/mcp-apps-server) before
deciding whether to retry.

### 4. Verify documentation and record the release

Confirm that both crates.io pages list the prepared version. docs.rs automatically
builds documentation after publication; it may remain queued for a while. Select
the new version in the [runtime API reference](https://docs.rs/mcp-apps-server)
and [macro API reference](https://docs.rs/mcp-apps-server-macros). If documentation
is unavailable, inspect that version's **Builds** page to distinguish waiting from
a failed build. See [docs.rs build guidance](https://docs.rs/about/builds) for
diagnosis and rebuild requests.

After both uploads succeed, create and push an annotated tag for the recorded
source commit:

```powershell
git tag -a "v$releaseVersion" $releaseCommit -m "Release v$releaseVersion"
git push origin "v$releaseVersion"
```

Keep an existing release tag on its original published commit. Later documentation
updates belong in new commits on `main` and will enter Cargo archives in a future
release.

Optionally create a [GitHub Release](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)
from that tag, with the relevant changelog entry and links to both crates.io
packages and their documentation. Mark alpha versions as prereleases. A Git tag
identifies a commit; a GitHub Release adds a reader-facing page of release notes.

See the [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
and [`cargo publish` reference](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
for registry behavior and command options.
