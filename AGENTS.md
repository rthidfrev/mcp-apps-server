# Repository guidelines

## Project and scope

MCP Apps Server is a Rust library project for server-side helpers for the MCP Apps
extension, intended to integrate with `rmcp`. Keep generic MCP Apps responsibilities
separate from host-specific extensions and application-domain rules.

The library prepares declarations and resources and supplies generic application
registration and tool/resource routing. A ready server adapter handles the common
MCP Apps path; a consuming server can invoke the same router from its own handlers.
The consuming server owns application services, access controls and transport
lifecycle. The browser application renders the interface; the host displays it
and mediates communication.
Do not move browser rendering or host behavior into this library without an
explicitly agreed change of scope.

Read the README for the current implementation status. Use Cargo.toml for actual
package, compiler and feature declarations. A roadmap, example proposal or planned
WorkCase result is not evidence that an API exists.

## Authority and working method

- Establish the requested result, permitted effects and behavior to preserve before
  changing files. Inspect affected contracts, implementation and consumers.
- A request to discuss, investigate, design or review authorizes that work, not an
  implementation or publication. Complete authorized work autonomously within its
  mandate. Ask for a decision when a consequential choice is reserved to the user;
  stop the current assignment while awaiting a genuinely required human answer.
  Ask directly without an expiring asynchronous question; preserve state and
  interrupt affected agents. Do not create questions for routine choices that
  are already delegated or facts that can be investigated within the mandate.
- Keep confirmed requirements, observations, hypotheses, proposals and decisions
  distinct. Check product- or version-dependent claims against primary sources.
- Make coherent changes. Separate unrelated refactoring from behavior changes and
  preserve unrelated user work. Add abstractions only for a concrete responsibility
  or use case; avoid speculative extension points and general-purpose frameworks.
- Reconcile uncertain external effects before retrying. A failed or lost response
  does not prove that an operation had no effect.
- When work is tracked, recover its mandate and actual state, record
  useful findings and dependencies, and appraise results before marking work
  complete. Tracking records are not authorization to expand the assignment.

## Language and communication

- Write maintained project content in English:
  identifiers, comments, documentation, error messages, commits and contribution
  descriptions. Preserve quoted or imported sources in their original language.
- Use concrete, consistent vocabulary. Explain significant changes, the reason for
  them, the evidence obtained and remaining limits. Distinguish proposed,
  implemented, executed, verified and published results.
- Keep prose concise and self-contained. Define necessary technical terms; do not
  rely on labels such as "clean code" or "production grade" to specify a rule.

## Rust implementation and public API

- Follow Rust naming conventions: `snake_case` for modules, functions and variables,
  `UpperCamelCase` for types and traits, and `SCREAMING_SNAKE_CASE` for constants.
  Choose names that communicate responsibility and use the same terms throughout.
- Apply the same naming, conversion, validation, error and visibility conventions
  to analogous responsibilities. Different ownership, lifecycle or contract needs
  may justify a different implementation; retain the reason when it matters to
  maintenance. Do not impose identical shapes on different responsibilities.
- Use Rust's ownership and borrowing to make resource use explicit. Prefer borrowing
  when ownership transfer is unnecessary. Justify cloning, shared ownership,
  allocation, dynamic dispatch and generic bounds through the actual API needs.
  Do not add traits, derives or bounds that the operation does not require.
- Use standard conversion traits where their meaning fits: `From` for infallible
  ownership conversions and `TryFrom` for fallible conversions. Follow the usual
  `as_`, `to_` and `into_` naming for borrowed views, conversions and consuming
  conversions. Do not hide validation failures in an infallible conversion.
- Keep implementation modules and fields private by default. Expose only types,
  operations and guarantees useful to consumers. Review public traits, fields,
  variants and dependency types as compatibility commitments. Apply builders,
  sealed traits or `#[non_exhaustive]` only when their trade-offs serve that API.
- Validate externally supplied values at the boundary responsible for their
  contract. Return meaningful errors for recoverable failures and preserve relevant
  causes. Do not turn transport failures into invalid-input errors or uncertain
  outcomes into false success. Avoid `unwrap`/`expect` on caller-controlled data or
  fallible external operations; any invariant-based panic needs a justified and
  documented invariant.
- For fixed protocol shapes, prefer explicit typed representations with deliberate
  serialization mappings. Use dynamic JSON for genuinely open or dynamic data.
  Preserve unrelated metadata and unknown fields where the extension contract
  requires it; changing a Rust name must not silently change a protocol field.
- Do not introduce process-global state, a runtime, filesystem effects, logging
  configuration or hidden background work unless the agreed library contract
  requires it. Let consumers retain control over their server lifecycle.
- Unsafe code is forbidden in every project-owned Rust crate. Enforce
  `#![forbid(unsafe_code)]` at each crate root and do not weaken it. This rule does
  not claim that dependencies contain no unsafe implementation internally.

## Modules and dependencies

- Organize modules around coherent responsibilities and useful navigation. Use a
  small flat layout until a grouping has a clear purpose. A folder, trait or file
  per type is not a goal. Avoid arbitrary function-size or file-count rules.
- The runtime package and optional `derive/` procedural macro package share a
  Cargo workspace. Keep further packages justified by a concrete responsibility.
- Before adding or replacing a dependency, establish its concrete need, existing
  alternatives, maintenance and release history, API maturity, compiler support,
  license compatibility, known security advisories and transitive cost. Contributor
  count alone neither establishes reliability nor disqualifies a crate. Record
  consequential reasons in the work or contribution, without creating a report for
  every routine choice.
- Prefer supported, stable releases. Do not introduce an unreviewed Git dependency,
  prerelease or toolchain installation to bypass a compatibility problem.
- Keep core functionality directly usable. Put genuinely optional convenience
  facilities, such as derive macros, behind documented Cargo features and optional
  dependencies. Features should add capabilities and remain usable together; do
  not introduce mutually exclusive features without a demonstrated necessity.
- Consider dependency types exposed in public signatures and changes to default
  features part of compatibility review. See RELEASING.md for release policy.

## Documentation and comments

- Keep user-facing API documentation in Rust source using `//!` for crate/module
  introductions and `///` for items. Document every externally reachable public
  item. Introduce its purpose first, then explain non-obvious semantics, effects,
  errors, panic conditions and required features where applicable.
- Supply realistic, usable examples for public operations and typical workflows.
  Prefer examples that run as documentation tests. Use `no_run` or `compile_fail`
  only for a real reason, and explain the limitation. Avoid `ignore` as a substitute
  for repairing an example. Do not invent an API to fill an initialization README.
- Scale private documentation to maintenance needs. Explain non-obvious invariants,
  ownership, protocol requirements and design choices; do not repeat every
  implementation detail or apply public-user tutorials to private helpers.
- The README is the user entry point: purpose, status, getting started, practical
  navigation and links to the API reference. Keep detailed API contracts in rustdoc
  rather than maintaining a second reference in the README.
- Apply Diataxis by keeping learning tutorials, task-oriented how-to guidance,
  factual reference and explanations distinguishable. Add documents when actual
  content needs them; do not create four empty documentation trees.
- Ordinary comments explain a reason, constraint or non-obvious behavior that code
  alone does not communicate. Do not narrate obvious assignments or control flow.
- Update affected documentation and examples with the code. Use relative Markdown
  links for repository files and rustdoc item links for API relationships. Keep
  examples, commands and capability claims accurate for the configurations shown.

## Tests and evidence

- Justify each added test through a behavior or contract it protects, a credible
  defect or regression it can detect, and an expectation supported independently
  of the implementation. A simple implementation can deserve a regression test;
  difficult-looking code does not automatically justify one.
- Choose the least costly level that preserves the behavior being examined,
  including execution, diagnosis and maintenance cost. Unit tests isolate local
  behavior; integration tests exercise cooperating components; end-to-end tests
  exercise the complete declared path. Names alone do not establish that scope.
- Favor contractual outcomes, meaningful protocol shapes, invalid inputs and
  consequential boundaries. Do not mirror trivial getters, internal layouts or
  implementation steps unless they are themselves contractual. Avoid redundant
  cases and fixtures; neither test counts nor coverage percentages are goals.
- Run justified deterministic local tests and executable documentation examples
  automatically within the mandate. Do not add demo arithmetic tests or tests of
  Markdown wording merely to make initialization appear tested.
- Define real-host, interactive or environment-changing experiments with the
  maintainer before execution. The protocol must identify the question, environment, inputs,
  human actions, observations, expected distinctions and relevant cleanup. Conduct
  those experiments with the agreed operator; do not invent substitute host behavior
  or modify configuration to conduct them autonomously.
- Never weaken an expectation only to obtain a pass. Distinguish local checks,
  simulations and real-host observations. Report unperformed checks and limits;
  passing tests establish only what their cases and conditions actually examine.

## Required local checks

For Rust changes, run the following from the package root:

```text
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Also run the tests, example checks and documentation generation relevant to the
change. Use `cargo doc --locked --no-deps --all-features` with
`RUSTDOCFLAGS="-D warnings"` for API documentation changes, and `cargo test --locked
--doc --all-features` when executable examples exist. Check the declared minimum
supported Rust version when establishing or changing support claims. Check meaningful
default, minimal or feature combinations when they differ; do not assume
`--all-features` validates every supported configuration.

Fix relevant failures and record justified, narrowly scoped lint exceptions. Do not
enable every Clippy lint group indiscriminately, suppress an unexplained warning or
claim that formatting and linting prove functional correctness. Run these commands
once Cargo.toml exists; explain unavailable checks during documentation-only work.

## Repository hygiene and Git

- Keep credentials and sensitive data out of source, documentation, fixtures,
  diagnostics and shared artifacts. Collect only data needed for the task.
- Confirm the repository, branch and working tree before changes and commits.
  Preserve unrelated modifications, secrets and local configuration.
- Use .gitignore for shared build outputs, disposable files and local project state.
  Use .git/info/exclude for personal editor or machine-specific files. Preserve
  local project state and any needed evidence while excluding them from commits
  and Cargo packages. Do not delete them as a substitute for exclusion.
- Keep Cargo.lock versioned here to reproduce contributor checks. It does not fix
  dependency resolution in consuming applications; review supported dependency
  ranges separately when dependencies are introduced.
- Stage explicit files or selected hunks and inspect the exact staged diff before
  committing. Make coherent commits with English messages that explain the change.
  Include the necessary documentation and justified checks with the change.
- Commit only within the user's authorized scope. Pushing, creating remote
  repositories, tagging and publishing require authorization for those effects.
  Do not rewrite history, discard work or force-push without explicit permission.
- Follow RELEASING.md for compatibility classification and release preparation.
  Report the resulting behavior, checks actually executed, compatibility impact and
  unresolved work without confusing a local commit with a published release.
