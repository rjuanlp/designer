# DSG format — current v4 subset

The engine reads and writes ZIP archives containing UTF-8 `manifest.json`
and `document.json`. The manifest has `format: "designer-dsg"`,
`schema_version: 4` and a generator string. Document IDs survive save/open.
The reader remains compatible with versions 1 to 3; new writes use version 4
for frame clipping. Frame kinds contain `clips_content: bool`; an absent field
defaults to false, preserving unclipped rendering of older documents.
Interactive prototype states and transitions remain supported. Documents are validated
before writing and before becoming active state.

Contract coverage includes reading a generated version 3 archive with the
clipping field omitted, and a version 4 write/read round trip with clipping
enabled. Text-field editing state is transient and adds no archive fields.

The reader rejects malformed ZIP/JSON, missing entries, unsafe archive paths,
invalid model values and future schema versions. Limits: 256 MiB for an on-disk
archive, 64 MiB uncompressed document, 64 KiB manifest, 4096 entries.
Save writes a sibling temporary file, flushes it and renames it over the
previous destination. Failed saves leave the session dirty and preserve the
previous destination. Contract tests cover replacement and failed writes.

This is a partial implementation: assets, recovery snapshots,
migration, published JSON schemas and interrupted-save recovery are not
implemented. Do not treat the format as frozen or use simultaneous writers.

See `crates/designer-engine/tests/dsg_contract.rs` for generated fixtures and
round-trip, corrupt, future-version, invalid-model and unsafe-path tests.

## `Session` locks

A file-backed `Session` holds an exclusive OS advisory lock on `<name>.dsg.lock`
beside the document. The PID is diagnostic only; stale sidecars do not block
opening. Process exit releases the OS lock automatically. Opening locks before
reading; Save As acquires the new lock before writing and retains the old lock
until success. Repeated own-path saves reuse the lock. Contention returns
`DsgError::Locked` without modifying the file or either session. Drop attempts
sidecar removal while still locked; cleanup is best-effort. Do not delete live
sidecars. Low-level archive helpers do not provide session ownership.

Sidecars are removed best-effort only on Windows. On other platforms they
remain in place to avoid an unlink race between waiting and new lock owners.
