Title: S2 – macOS write-time dedupe: prefer PNG over RGBA

Goal
If both PNG and RGBA forms of the same image are about to be written to the pasteboard, only publish PNG. This reduces ambiguous multiple items and helps consumers pick the transparency-safe format.

Change summary
- File: src/platform/osx.rs
- In Set::formats, detect presence of ImageData::Png; if present, skip ImageData::Rgba entries.
- All new logs include the prefix ======================

Build & test
- Windows: cargo check passes (macOS code is behind cfg).
- macOS: paste various screenshots into Notes/Messages/Pages and verify a single item is written; check with the Swift pasteboard dumper (see S1 README) that items/types look expected.

Notes
- This branch does not attempt to merge multiple representations into one item (see S3 for that experiment).
- No Cargo.toml changes.
