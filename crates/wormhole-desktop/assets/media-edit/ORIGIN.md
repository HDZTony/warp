# Media edit icons

Copied from Telegram Desktop `Telegram/Resources/icons` for PhotoEditor UI parity.

| Wormhole asset | Telegram source |
|----------------|-----------------|
| `media-edit-flip.png` | `photo_editor/flip@2x.png` |
| `media-edit-rotate.png` | `photo_editor/rotate@2x.png` |
| `media-edit-paint.png` | `photo_editor/paint@2x.png` |
| `media-edit-undo.png` | `photo_editor/undo@2x.png` |
| `media-edit-stickers.png` | `settings/settings_stickers@2x.png` |
| `media-edit-ratio.svg` | `photo_editor/ratio.svg` |
| `media-edit-shapes.svg` + `shape_*.svg` | `photo_editor/shapes.svg` / `shape_*.svg` |
| `media-edit-undo.svg` / `redo.svg` | `menu/tabs_undo.svg` / `tabs_redo.svg` |

PNG glyphs are white alpha masks; Warp `Icon` tints them (inactive white / active accent).

Bar metrics match `editor.style`: `photoEditorButtonBarWidth` 422, height 48, mid IconButtons flush (no extra gap).

Telegram Desktop is GPL-3.0-or-later; these assets are used for visual alignment in Wormhole desktop chat media editor.
