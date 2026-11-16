# MCP integration — draft contract

**Not implemented.** There is no usable designer-mcp binary yet. Do not configure
a client to launch it until the MCP milestone is complete.

The planned Rust rmcp stdio server delegates to designer-engine through a
current-user-only Windows named pipe. Protocol messages go to stdout; diagnostics
go to stderr. The GUI and all agent connections share the engine's document state.

Mutation inputs identify document, expected revision and typed operation.
Transactions either apply in full or leave the document unchanged. Results carry
the new revision, affected IDs and diagnostics. Revision conflicts require a fresh
read, not a blind overwrite. One transaction produces one undo entry.

Capabilities: document/page/node CRUD, geometry, text, layout, components, styles,
assets, selection/view, undo/redo, keyframes, curves, sequences, morphing, states,
structured import and save. Read tools expose the tree and properties; render
tools provide still snapshots at an explicit time. No shell or script execution.

A capability registry will generate tool schemas and coverage tests. Headless
editing must not need a GPU; rendering must report a missing compatible adapter.

The implementation milestone must add tested Codex and Claude Code configuration
examples with absolute executable paths, initialization/tool-list/tool-call tests,
malformed-input tests and a complete create/animate/save/reopen example. This
draft deliberately does not invent tool names that an agent cannot call.
