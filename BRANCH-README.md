Title: S4 – Windows image reader: prefer PNG; add CF_DIB fallback; force alpha=0xFF for BI_RGB 32bpp

Changes
- Get::image_ order: SVG → PNG → DIBV5 → DIB.
- When reading CF_DIB (BI_RGB 32bpp), assume alpha is undefined and force it to 0xFF after converting to RGBA.
- Debug logs with the prefix ======================= indicate which Windows format was used.

Files touched
- src/platform/windows.rs (Get::image_ and image_data::read_cf_dib)

Build & test (Windows)
- cargo build / cargo run
- Copy screenshots via Win+Shift+S; paste to macOS; confirm transparent areas look correct in combination with S1/S3.

Notes
- No Cargo.toml changes.
