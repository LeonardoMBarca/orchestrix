# Help center illustrations

These six SVG files illustrate tutorial concepts using the shared Orchestrix navy and indigo palette. Updated on **2026-10-12**, they are code-native diagrams with searchable text, not captures of account data. The external [Help center](../../help.html) uses them in its ten English written guides and works over HTTP or direct `file://` opening.

| File | Concept and searchable terms | Used in | Dimensions |
| --- | --- | --- | --- |
| [session-tree.svg](session-tree.svg) | Fixed left activity rail, Sessions on the right, project groups, independent conversation, session history, attach session | Sessions & projects | 920 × 440 |
| [workspace-docking.svg](workspace-docking.svg) | Fixed activity rail, direct Chat without its own sidebar, Files panel left, independent Sessions right, shared Files/Sessions tabs, resize edge, collapse and restore, keyboard | Arrange your workspace | 920 × 410 |
| [settings-window.svg](settings-window.svg) | Centered floating Settings, outside click, conversation preserved, General, Compact density, text percentage slider | Settings & personal style | 920 × 440 |
| [account-usage.svg](account-usage.svg) | Subscription quota, API spending, unknown usage, reported limits, Subscription Only | Accounts & usage | 920 × 370 |
| [review-path.svg](review-path.svg) | Request, work, review, correction loop, validation, application confirmation | Progress & review | 920 × 330 |
| [files-editor.svg](files-editor.svg) | Files tree, .env, Markdown tab, text editor, unsaved changes, authorized folder, Save file, imported folder, Save a copy | Files & quick edits | 920 × 480 |

Do not replace unknown quota or balance with a made-up number. Keep the diagrams schematic, concise, and consistent with actual application labels when they change. Each image has SVG title/description and a contextual HTML alternative in the guide.

Local visual checks are stored under `prototypes/desktop/artifacts/` and are not versioned:

| Capture | Contents | Viewport |
| --- | --- | --- |
| `help-center-desktop.png` | Header, guide search, topic navigation, first-session instructions | 1440 × 1000 |
| `help-center-mobile.png` | Compact header, search, wrapping topic links | 390 × 844 |
| `help-center-docking.png` | Historical docking chapter before bottom docking was removed | 1440 × 1000 |
| `help-center-accounts.png` | Unknown subscription/API usage and billing-boundary explanation | 1440 × 1000 |
| `session-tree-help.png` | Updated session diagram with fixed rail and Sessions on the right | 920 × 440 |
| `workspace-docking-help.png` | Side docks, shared tabs, collapsed edges and keyboard instructions before the direct-Chat label refinement | 920 × 410 |
| `files-editor-help.png` | Files diagram with source-save and copy-save paths | 920 × 480 |

## Video publishing

The `quickstart`, `sessions`, `files`, and `review` guides reserve hidden video slots. Fill `videoGuides` in [help.js](../../help.js) with a real `src`, optional WebVTT `captions`, and optional transcript page/file. Relative paths resolve from `help.html`; HTTP(S) and direct file URLs are accepted. Empty slots remain hidden and create no player or requests. A configured video uses native controls, does not autoplay, and leaves the written guide available if loading fails.

For HTTP preview, add any local media path to the development server allowlist. For publication, serve the media and transcript with the appropriate content types. Keep captions and transcripts available for video-only information.
