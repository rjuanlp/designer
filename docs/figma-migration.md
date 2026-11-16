# Migrating a Figma design with an AI agent

**Preparation guide. Designer's import command is not implemented yet.**

Designer will read a local structured export, not binary .fig files. If you only
have a .fig file, first open it in Figma with an authorized account.

## Source preparation

1. Confirm you have permission to export the design and its assets.
2. Through the official Figma API, obtain the document JSON with geometry=paths.
   Preserve node IDs, hierarchy, local transforms, text styling, layout and
   component references. See https://developers.figma.com/docs/rest-api/file-endpoints/.
3. Retrieve referenced image assets through the supported API and store them
   locally. Download authorized assets before temporary URLs expire.
4. Record missing fonts, external components and unsupported effects. Never put
   access tokens in the migration bundle or Git history.
5. Preserve an original source snapshot and reference images for visual comparison.

## Agent handoff instructions

Give the agent the prepared document, assets and reference views. Require it to
inspect the destination's actual MCP tool catalogue before calling any tools.
Once structured import exists, import as a new document, examine its loss report,
and repair supported nodes through transactional commands. Preserve editable
hierarchy; never silently flatten an entire design to an image.

Compare representative frames at the same viewport. Check text metrics, layout,
clipping, gradients, images and component overrides. Report all unsupported
features. Save as .dsg, reopen, then verify the tree and rendered output.

Do not claim imported animation compatibility. Designer animations are authored
inside Designer and do not imply round-trip compatibility with Figma.

## Completion evidence

A successful migration includes the resulting .dsg, mapping/loss report, reopened
document validation and before/after screenshots. Until the importer exists,
agents can prepare source packages but cannot complete this workflow.
