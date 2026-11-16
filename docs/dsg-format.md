# DSG format — current v2 subset

The engine reads and writes ZIP archives containing UTF-8 `manifest.json`
and `document.json`. The manifest has `format: "designer-dsg"`,
`schema_version: 2` and a generator string. Document IDs survive save/open.
The reader remains compatible with version 1 documents; new writes use version
2 for path geometry and morph-track data. Documents are validated before
writing and before becoming active state.

The reader rejects malformed ZIP/JSON, missing entries, unsafe archive paths,
invalid model values and future schema versions. Limits: 256 MiB for an on-disk
archive, 64 MiB uncompressed document, 64 KiB manifest, 4096 entries.
Save writes a sibling temporary file, flushes it and renames it over the
previous destination. Failed saves leave the session dirty and preserve the
previous destination. Contract tests cover replacement and failed writes.

This is a partial implementation: assets, write locks, recovery snapshots,
migration, published JSON schemas and interrupted-save recovery are not
implemented. Do not treat the format as frozen or use simultaneous writers.

See `crates/designer-engine/tests/dsg_contract.rs` for generated fixtures and
round-trip, corrupt, future-version, invalid-model and unsafe-path tests.