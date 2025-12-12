Title: S1 – macOS prefer PNG when publishing images

Problem
Some macOS consumers default to TIFF from the pasteboard, which may expose alpha/premultiplication quirks when the source was RGBA. Ensuring a PNG representation is published generally fixes “white box/halo” around transparent regions when pasting Windows screenshots into macOS apps.

What this branch changes (macOS only)
- In src/platform/osx.rs Set::formats, for RGBA input we transcode NSImage → PNG via NSBitmapImageRep and write NSPasteboardTypePNG.
- If transcoding fails, we fall back to writing the NSImage object (old behavior).
- Item model is unchanged (this branch does not merge items).
- Added logs with a distinctive prefix: ======================

Build & run
- Windows: cargo check passes; macOS-only code is behind target_os = "macos".
- macOS: build arboard as usual; run RustDesk; test Win→Mac copy/paste for images.

How to verify on macOS
1) Paste into Notes / Messages / Pages / Slack and check that transparency is preserved.
2) Inspect pasteboard types with the tiny Swift helper below.

Swift helper (PasteboardDump.swift)
import AppKit
let pb = NSPasteboard.general
for (idx,item) in (pb.pasteboardItems ?? []).enumerated() {
    print("Item #\(idx):")
    for t in item.types { print("  type: \(t.rawValue)") }
}
// Build & run: swiftc PasteboardDump.swift -o pb && ./pb

Notes
- We intentionally avoid changing Cargo.toml. The PNG transcoding uses AppKit (NSBitmapImageRep) and does not pull new crates.
- SVG input is not rasterized.
