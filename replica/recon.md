# Screen and behavior inventory

Reference: public Figma Design help, viewed as user documentation, never source
code. No original application assets or copy are incorporated.

| ID | Surface | Purpose | Required states |
| --- | --- | --- | --- |
| S00 | GPU feasibility window | Validate GPUI/Vello composition | startup, running, resized, minimized, GPU lost |
| S01 | Local documents | Create/open/recover `.dsg` | empty, recent, loading, corrupt, permission denied |
| S02 | Design workspace | Pages, layers, canvas, properties | empty, selected, multi-selected, locked, long names |
| S03 | Components and styles | Local reusable design system | empty, populated, missing dependency, overrides |
| S04 | Animation timeline | Keys, curves, nested sequences | empty, playing, paused, invalid morph, conflicting tracks |
| S05 | Interactive preview | States and triggers | initial, hover, click, interrupted transition |
| S06 | Import and migration | Map structured source data | loading, success, partial, invalid, missing assets |
| S07 | Preferences and agents | Theme, accessibility, connections | connected, disconnected, conflict, unavailable GPU |

Core loop: create document, draw frame and text, animate, play, save/reopen,
modify via MCP, undo. Test 1440 and 390 logical-pixel widths, keyboard navigation,
visible focus, accessible labels, long names, empty/error/loading states.
