use arboard::{Clipboard, ClipboardData, ClipboardFormat};

fn main() {
	env_logger::init();
	let mut clipboard = Clipboard::new().unwrap();
	println!(
		"Clipboard urls was: {:?}",
		clipboard.get_formats(&vec![ClipboardFormat::FileUrl]).unwrap()
	);
}
