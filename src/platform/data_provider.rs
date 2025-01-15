use objc2::{
	declare_class, msg_send_id, mutability,
	rc::Id,
	runtime::{AnyObject, NSObject, NSObjectProtocol},
	ClassType, DeclaredClass,
};
use objc2_app_kit::{
	NSPasteboard, NSPasteboardType, NSPasteboardWriting, NSPasteboardWritingOptions,
};
use objc2_foundation::{NSArray, NSString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlatformDataProvider {
	r#type: &'static NSPasteboardType,
	data: String,
}

impl PlatformDataProvider {
	pub(crate) fn new(r#type: &'static NSPasteboardType, data: String) -> Self {
		Self { r#type, data }
	}

	pub(crate) fn create_writer(self) -> Id<SNEPasteboardWriter> {
		let writer = SNEPasteboardWriter::alloc();
		let writer = writer.set_ivars(self);
		unsafe { msg_send_id![super(writer), init] }
	}

	fn writable_types(&self) -> Id<NSArray<NSPasteboardType>> {
		NSArray::from_vec(vec![self.r#type.to_owned()])
	}

	fn object_for_type(&self, pasteboard_type: &NSPasteboardType) -> Option<Id<NSObject>> {
		let data = &self.data;
		if pasteboard_type == self.r#type {
			Some(unsafe { Id::cast(NSString::from_str(data)) })
		} else {
			None
		}
	}
}

declare_class!(
	pub(crate) struct SNEPasteboardWriter;

	unsafe impl ClassType for SNEPasteboardWriter {
		type Super = NSObject;
		type Mutability = mutability::InteriorMutable;
		const NAME: &'static str = "SNEPasteboardWriter";
	}

	impl DeclaredClass for SNEPasteboardWriter {
		type Ivars = PlatformDataProvider;
	}

	unsafe impl NSObjectProtocol for SNEPasteboardWriter {}

	unsafe impl NSPasteboardWriting for SNEPasteboardWriter {
		#[method_id(writableTypesForPasteboard:)]
		#[allow(non_snake_case)]
		unsafe fn writableTypesForPasteboard(
			&self,
			_pasteboard: &NSPasteboard,
		) -> Id<NSArray<NSPasteboardType>> {
			println!("REMOVE ME ======================= writableTypesForPasteboard");
			self.ivars().writable_types()
		}

		#[method(writingOptionsForType:pasteboard:)]
		#[allow(non_snake_case)]
		unsafe fn writingOptionsForType_pasteboard(
			&self,
			r#_type: &NSPasteboardType,
			_pasteboard: &NSPasteboard,
		) -> NSPasteboardWritingOptions {
			println!("REMOVE ME ======================= writingOptionsForType_pasteboard");
			NSPasteboardWritingOptions::NSPasteboardWritingPromised
		}

		#[method_id(pasteboardPropertyListForType:)]
		#[allow(non_snake_case)]
		unsafe fn pasteboardPropertyListForType(
			&self,
			r#type: &NSPasteboardType,
		) -> Option<Id<AnyObject>> {
			println!("REMOVE ME ======================= pasteboardPropertyListForType");
			self.ivars().object_for_type(r#type).map(|v| Id::cast(v))
		}
	}

	unsafe impl SNEPasteboardWriter {}
);
