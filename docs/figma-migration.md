# Migrating a Figma design with an AI agent

Designer can import the supported subset of a local structured Figma JSON
export from the Import screen or through the `import_figma_json` MCP tool.
Import is not lossless; review the report before relying on the result.

Designer will read a local structured export, not binary .fig files. If you only
have a .fig file, first open it in Figma with an authorized account.

## Source preparation

1. Confirm you have permission to export the design and its assets.
2. Export the document JSON through Figma's supported tools. Keep node IDs,
   hierarchy, text styling, layout and component references in the source;
   the importer maps IDs but only preserves supported geometry and solid fills.
   See https://developers.figma.com/docs/rest-api/file-endpoints/.
3. Preserve referenced image assets locally for manual rebuilding. The
   importer does not bring images or component assets across.
4. Record missing fonts, external components and unsupported effects. Never put
   access tokens in the migration bundle or Git history.
5. Preserve an original source snapshot and reference images for visual comparison.

## Agent handoff instructions

Give the agent the prepared document, assets and reference views. Require it to
inspect the destination's actual MCP tool catalogue before calling any tools.
Import as a new document and examine its loss report before continuing. The
importer supports frames/groups, rectangles, ellipses, text strings, absolute
bounds and the first visible solid fill. It warns about unsupported geometry,
effects, strokes, text styling, rotation, auto-layout and constraints; missing
or unrepresentable values are normalized and reported. Oversized names and
text are truncated with a warning. Preserve editable hierarchy; never silently
flatten an entire design to an image.

Compare representative frames at the same viewport. Check text metrics, layout,
clipping, gradients, images and component overrides. Report all unsupported
features. Save as .dsg, reopen, then verify the tree and rendered output.

Do not claim imported animation compatibility. Designer animations are authored
inside Designer and do not imply round-trip compatibility with Figma.

## Completion evidence

A successful migration includes the resulting .dsg, mapping/loss report, reopened
document validation and before/after screenshots. This importer is a migration starting point, not a round-trip Figma bridge.
Binary `.fig` files, gradients, images, fonts, text styles, components,
constraints, effects and animation are not imported.
