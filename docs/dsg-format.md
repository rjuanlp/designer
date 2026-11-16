# DSG format — draft v1

**Specification draft, not implemented.** Native extension: .dsg.

The planned container is ZIP with UTF-8 manifest.json and document.json plus
assets addressed by SHA-256. The manifest identifies the format as Designer DSG
and contains an integer schema version. The document holds pages, typed nodes,
styles, components, animation tracks, sequences and interactive states. IDs remain
stable across save/reopen. Source IDs from migrations are metadata.

A reader must validate format identity, schema version, reference integrity,
finite numbers, hierarchy/sequence acyclicity, asset hashes, archive paths and
bounded uncompressed size before publishing a document. Unknown future major
versions are rejected without overwriting the source. Do not deserialize invalid
values directly into active state.

Save writes a sibling temporary file, flushes it, then atomically replaces the
destination. The engine owns a write lock. Recovery snapshots are separate from
the last successful user save. Failed writes preserve the previous file.

Before claiming v1 support, publish machine-readable schemas and fixtures for
round-trip, corrupt archives, future versions, missing resources and interrupted
save recovery. No sample .dsg is shipped until the writer and reader are tested.

Forks must namespace additional metadata and document version changes. Do not
reuse a version number for incompatible semantics. This specification will be
frozen only after the persistence milestone passes.
