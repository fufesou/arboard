Title: S3 – macOS single item with multiple image representations (PNG + TIFF + optional SVG)

Goal
Publish a single NSPasteboardItem for image content and attach multiple types so consumers can choose: PNG (always), TIFF (optional), and SVG (if present).

What changed
- File: src/platform/osx.rs, Set::formats:
  - Aggregate image payloads into one NSPasteboardItem (`image_item`).
  - Always set NSPasteboardTypePNG.
  - Unless `RUSTDESK_PB_NO_TIFF=1`, also attach `NSPasteboardTypeTIFF`.
  - For RGBA: create NSImage from pixels, derive PNG via NSBitmapImageRep; attach PNG (+TIFF when enabled).
  - For PNG: reuse bytes; derive TIFF via NSBitmapImageRep.
  - For SVG: attach the SVG string type; no rasterization is performed in this branch.
- Text/RTF/HTML/Special remain in a separate "main" item as before.
- Logs use the prefix `======================`.

Build & test
- Windows: `cargo check` (macOS code is behind cfg).
- macOS: run and paste into Notes/Messages/Pages/Slack. With default settings, expect one item with types: `public.png` and `public.tiff`.
- To disable TIFF, set `RUSTDESK_PB_NO_TIFF=1` and repeat tests.

Swift helper (PasteboardDump.swift)
import AppKit
let pb = NSPasteboard.general
for (idx,item) in (pb.pasteboardItems ?? []).enumerated() {
    print("Item #\(idx):")
    for t in item.types { print("  type: \(t.rawValue)") }
}

Notes
- No Cargo.toml changes; AppKit’s NSBitmapImageRep is used for transcoding.
- SVG rasterization is intentionally out of scope for this branch.
