# MCP integration

`designer-mcp` is a local, headless MCP server using the `rmcp` stdio
transport. Build it with `cargo build --release -p designer-mcp`, then launch
the resulting executable directly from the MCP client. It opens the sample
document by default or a `.dsg` file supplied with `--open`.

## Tools

| Tool | Purpose |
| --- | --- |
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

The server process owns one in-memory `Session`. This build does **not** yet
share live state with the GPUI editor or coordinate independent server
processes, does not provide a named pipe, and does not provide a cross-process
write lock. Do not edit the same `.dsg` concurrently in Designer and MCP or
from two MCP processes. Restart with `--open` to reopen a saved document.

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
args = ["--open", "C:\\Users\\YOUR_USER\\Documents\\design.dsg"]
```

Claude Code `.mcp.json`:

```json
{
  "mcpServers": {
    "designer": {
      "command": "C:\\Users\\YOUR_USER\\source\\Designer\\target\\release\\designer-mcp.exe",
      "args": ["--open", "C:\\Users\\YOUR_USER\\Documents\\design.dsg"]
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
registration and atomic transaction failure. Since the
GPUI 0.2.2 fork exposes no accessible roles or labels for its custom-rendered
elements, screen-reader support remains an open accessibility requirement.
