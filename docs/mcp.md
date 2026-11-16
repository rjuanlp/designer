# MCP integration

`designer-mcp` is a local MCP server using the `rmcp` stdio
transport. Build it with `cargo build --release -p designer-mcp`, then launch
the resulting executable directly from the MCP client. It opens the sample
document by default or a `.dsg` file supplied with `--open`.

## Tools

| Tool | Purpose |
| --- | --- |
| `describe_capabilities` | Transport mode; written schema; operation discriminators; prototype triggers; unsupported features. |
| `read_document` | Read the full document, current revision, file path, dirty state and history availability. |
| `apply_transaction` | Apply an array of typed `designer_core::Operation` values atomically, guarded by `expected_revision`. |
| `new_document` | Replace the current document with a new empty document as one undoable change. |
| `undo` / `redo` | Move history at an expected revision. |
| `save` / `save_as` | Save the current document as `.dsg`. |
| `import_figma_json` | Import supported structured JSON, replace the current document as one undoable change and return the loss report. |
| `render_snapshot` | Evaluate a page at an explicit time and return its still document tree without requiring a GPU. |

`apply_transaction` accepts the current serialized `Operation` enum, including
node geometry/fill/opacity, keyframes, nested sequences, morph keys and
interactive prototype states. A transaction either commits fully or leaves
document and history untouched; one transaction creates one undo entry. A
stale revision returns a conflict. Read the document again before retrying.
Malformed operations, invalid documents, import failures and save errors are
returned as tool errors. Stdio is reserved for MCP messages; diagnostics are
not written to stdout.

Without `--connect` the process owns a headless `Session`. On Windows, `--connect`
attaches to the running GUI over the local named pipe `\\.\pipe\designer-session-v1`.
`--pipe NAME` overrides the full pipe path and requires `--connect`. `--open` and
`--connect` are exclusive. Requests execute on the GUI thread with shared revision
and undo history. An absent server returns a tool error.

The GUI rejects mutations during a canvas drag or unsaved-changes prompt;
read-only tools remain available. Finish the interaction and reread before
retrying. A second GUI instance opens with an MCP-server-failed status. The
pipe rejects remote clients, bounds active connections to eight, limits
request size and applies a ten-second request read timeout.

File-backed Sessions hold exclusive OS sidecar locks. A headless `--open` cannot
independently open a document held by the GUI; use `--connect`. Save As locks the
destination before writing. Stale sidecars do not block access. See
[format](dsg-format.md).

The tools cover the current model only. Pages, components, styles, assets,
auto-layout, direct text editing, selection/view state and GPU image rendering
are not implemented in the model and are not advertised as available MCP
capabilities. `render_snapshot` returns evaluated model data, not a PNG.
There is no shell or script execution tool.

## Client configuration examples

Replace `YOUR_USER` and the example document path with local absolute paths.
First build the release binary; the examples do not invoke Cargo or require a
shell wrapper.

Codex `config.toml`:

```toml
[mcp_servers.designer]
command = "C:\\Users\\YOUR_USER\\source\\Designer\\target\\release\\designer-mcp.exe"
args = ["`--connect`"]
```

Claude Code `.mcp.json`:

```json
{
  "mcpServers": {
    "designer": {
      "command": "C:\\Users\\YOUR_USER\\source\\Designer\\target\\release\\designer-mcp.exe",
      "args": ["`--connect`"]
    }
  }
}
```

## Basic workflow

1. Start the server for the target `.dsg` and call `read_document`.
2. Apply one or more typed operations with the returned `revision`.
3. If a revision conflict occurs, reread and reconcile; never blindly replay a
   stale transaction.
4. Call `save`, stop the server, then restart with the same `--open` path and
   verify the reopened document.

The stdio integration tests exercise initialization, `tools/list`,
`tools/call`, a successful transaction, a stale-revision error and a
create-animate-save-reopen round trip. Unit tests cover advertised tool
registration and atomic transaction failure. Windows integration tests also launch the real binary with
`--connect` `--pipe`, verify reads/transactions/conflicts/undo and missing-server
errors. Prototype overrides are tested through the MCP transaction handler.

AccessKit exposes all-screen navigation and controls, design layers and
read-only inspector values, prototype state selection, import summary,
theme/timeline toggles and polite status. Focus and stale-target handling are
supported; unchanged trees are not republished. Only root bounds are supplied.
Per-control rectangles are omitted. The tree and its actions were checked
with a UI Automation client; speech output in Narrator or NVDA has not been
checked.
