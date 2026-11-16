# Maintaining a fork

## Start

Fork the public repository when published, clone your fork, then follow
[build instructions](build.md). Read the [current status](../replica/features.csv)
before assuming features exist. MIT permits modification and redistribution;
keep copyright/license notices and all applicable third-party notices.

## Identity and coexistence

Before distributing a renamed build, update application name, executable/package
identifiers, icons and documentation. When engine IPC and persistence are added,
also change named-pipe prefixes, per-user data/cache/recovery directories and
file-association ProgIDs. Never share locks or recovery files with another fork.

Use .dsg only for compatible files. Namespace extension metadata; version any
incompatible semantics explicitly. Preserve unknown supported extension data
when feasible, and reject versions you cannot interpret instead of corrupting
documents.

## Upstream synchronization

Add the original repository as an upstream remote and fetch it. Integrate on a
branch, inspect changes, run all checks, then merge into your release branch.
Keep local changes small and record architectural divergence. For a GPUI patch,
record its exact upstream revision, license, patch series and interop tests.
Revalidate resource synchronization after every graphics dependency update.

## Releases

Build from a clean checkout with the pinned toolchain and Cargo.lock. Run unit,
integration, documentation and hardware tests. Package notices, checksums and
honest release notes. A fork's releases use its own repository and credentials;
PR checks must work without secrets. Never imply upstream endorsement.

See [contribution guidelines](../CONTRIBUTING.md) to submit improvements upstream.
