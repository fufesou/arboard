use arboard::{Clipboard, ClipboardFormat};

const FORMAT_SPECIAL: &str = "dyn.arboard.pecial.format";

fn main() {
	env_logger::init();

	let mut ctx = Clipboard::new().unwrap();

	let formats = [
		ClipboardFormat::Text,
		ClipboardFormat::Html,
		ClipboardFormat::Rtf,
		ClipboardFormat::ImageRgba,
		ClipboardFormat::ImagePng,
		ClipboardFormat::ImageSvg,
		#[cfg(any(target_os = "linux", target_os = "macos"))]
		ClipboardFormat::FileUrl,
		ClipboardFormat::Special(FORMAT_SPECIAL),
		ClipboardFormat::Special("com.teamviewer.TVClipboard"),
	];
	for d in ctx.get_formats(&formats).unwrap() {
		println!("data: {:?}", &d);
		match &d {
			arboard::ClipboardData::Special((format, data)) => {
				println!("special format: {}", format);
				// convert data to string
				let data = std::str::from_utf8(data).unwrap();
				println!("special data: {:?}", data);
			}
			_ => {}
		}
	}
}
