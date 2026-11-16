# Contributing

Welcome! Start with [build instructions](docs/build.md), [architecture](replica/architecture.md)
and the [feature matrix](replica/features.csv). The graphics gate is currently the
priority; do not implement a second UI framework to bypass it.

Discuss architectural changes in an issue before a large pull request. Include
a minimal reproduction for bugs and observable acceptance criteria for features.
Keep changes focused, add meaningful unit/integration tests and update the relevant
English documentation and matrix. Describe what actually ran, including hardware
for GPU tests. Do not replace missing results with claims.

Run formatting, warning-free Clippy, tests, doctests and warning-free Rustdoc.
Every public capability must eventually be accessible through the shared command
registry and MCP. Avoid panics in production paths. Explain invariants rather
than narrating syntax. Unsafe code requires an isolated graphics boundary and
documented safety arguments.

Original contributions are accepted under MIT, with no additional CLA. Preserve
third-party licenses. Use original or permissively licensed fixtures, never private
customer documents. AI-assisted contributions follow the same review and testing
requirements; contributors are responsible for their correctness and provenance.

Maintainers review correctness, accessibility, compatibility and performance.
A failed graphics gate must remain explicit. Small first contributions include
reproductions, documentation fixes and deterministic core tests.
