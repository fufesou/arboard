use std::{
	cell::{Cell, RefCell},
	collections::HashMap,
	ffi::{CStr, OsStr},
	fs::File,
	io::Write,
	mem::ManuallyDrop,
	os::unix::{
		ffi::OsStrExt,
		prelude::{FromRawFd, IntoRawFd},
	},
	path::PathBuf,
	rc::{Rc, Weak},
	sync::{Arc, Mutex},
};

use block2::{Block, RcBlock};
use irondash_message_channel::{
	value_darwin::ValueObjcConversion, IntoValue, IsolateId, Late, TryFromValue,
};
use irondash_run_loop::{platform::PollSession, RunLoop};
use objc2::{
	declare_class, extern_class, extern_methods, msg_send_id,
	mutability::{self, InteriorMutable},
	rc::{Allocated, Id},
	runtime::{AnyObject, Class, NSObject, NSObjectProtocol, ProtocolObject},
	ClassType, DeclaredClass,
};
use objc2_app_kit::{
	NSFilePromiseProvider, NSFilePromiseProviderDelegate, NSPasteboard, NSPasteboardType,
	NSPasteboardWriting, NSPasteboardWritingOptions,
};
use objc2_foundation::{
	ns_string, NSArray, NSDictionary, NSError, NSInteger, NSProgress, NSProgressKindFile, NSString,
	NSURL,
};
use once_cell::sync::Lazy;

use irondash_run_loop::spawn;

use async_trait::async_trait;
use irondash_message_channel::{
	AsyncMethodHandler, AsyncMethodInvoker, IntoPlatformResult, MethodCall, PlatformError,
	PlatformResult, RegisteredAsyncMethodHandler, Value,
};

use super::{
	api_model::{DataProvider, DataProviderHandle, DataProviderValueId, DataRepresentation},
	data_provider_manager::{
		PlatformDataProviderDelegate, VirtualFileResult, VirtualSessionHandle,
	},
	error::NativeExtensionsResult,
	log::OkLog,
	value_promise::{ValuePromise, ValuePromiseResult},
};

pub fn to_nserror(domain: &str, code: NSInteger, message: &str) -> Id<NSError> {
	unsafe {
		let user_info = NSDictionary::<NSString, AnyObject>::from_vec(
			&[ns_string!("NSLocalizedDescription")],
			vec![Id::cast(NSString::from_str(message))],
		);

		NSError::errorWithDomain_code_userInfo(&NSString::from_str(domain), code, Some(&user_info))
	}
}

pub fn path_from_url(url: &NSURL) -> PathBuf {
	let path: *const i8 = unsafe { url.fileSystemRepresentation() }.as_ptr();
	let path = unsafe { CStr::from_ptr(path) };
	let path = OsStr::from_bytes(path.to_bytes());
	path.into()
}

pub fn platform_stream_write(handle: i32, data: &[u8]) -> i32 {
	let mut file = ManuallyDrop::new(unsafe { File::from_raw_fd(handle) });
	match file.write_all(data) {
		Ok(_) => 1,
		Err(_) => 0,
	}
}

static FILE_PATHS: Lazy<Mutex<HashMap<i32, PathBuf>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn platform_stream_close(handle: i32, delete: bool) {
	unsafe { File::from_raw_fd(handle) };
	let path = FILE_PATHS.lock().unwrap().remove(&handle);
	if let Some(path) = path {
		if delete {
			std::fs::remove_file(path).ok();
		}
	}
}

#[derive(Debug, TryFromValue, IntoValue, Clone, PartialEq, Eq)]
pub struct PlatformDataProvider {
	data: DataProvider,
}

thread_local! {
	static WAITING_FOR_PASTEBOARD_DATA: Cell<bool> = const { Cell::new(false) };
}

impl PlatformDataProvider {
	pub fn new(data: DataProvider) -> Self {
		Self { data }
	}

	pub fn set_waiting_for_pasteboard_data(waiting: bool) {
		WAITING_FOR_PASTEBOARD_DATA.with(|f| f.set(waiting));
	}

	pub fn is_waiting_for_pasteboard_data() -> bool {
		WAITING_FOR_PASTEBOARD_DATA.with(|f| f.get())
	}

	/// If retain_handle is false, writer will not retain the DataProviderHandle. This is useful
	/// for drag and drop where the item will live in dragging pasteboard after drag sessions is done.
	pub fn create_writer(&self, is_for_dragging: bool) -> Id<NSObject> {
		let state = Rc::new(ItemState {
			data_provider: self.clone(),
			is_for_dragging,
		});
		state.create_item()
	}

	pub fn write_to_clipboard(
		providers: Vec<Rc<PlatformDataProvider>>,
	) -> NativeExtensionsResult<()> {
		let items: Vec<_> = providers.into_iter().map(|p| p.create_writer(false)).collect();
		let array = NSArray::from_vec(items);
		let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
		unsafe { pasteboard.clearContents() };
		unsafe { pasteboard.writeObjects(&Id::cast(array)) };
		Ok(())
	}

    pub fn write_to_clipboard2(
        pasteboard: Id<NSPasteboard>,
		providers: Vec<Rc<PlatformDataProvider>>,
	) -> NativeExtensionsResult<()> {
		let items: Vec<_> = providers.into_iter().map(|p| p.create_writer(false)).collect();
		let array = NSArray::from_vec(items);
		unsafe { pasteboard.clearContents() };
		unsafe { pasteboard.writeObjects(&Id::cast(array)) };
		Ok(())
	}
}

pub struct ItemState {
	data_provider: PlatformDataProvider,
	is_for_dragging: bool,
}

extern "C" {
    fn majorVersion() -> u32;
}

struct VirtualFileInfo {
	id: DataProviderValueId,
	format: String,
}

impl ItemState {
	fn create_item(self: Rc<Self>) -> Id<NSObject> {
		let writer = SNEPasteboardWriter::alloc();
		let writer = writer.set_ivars(Ivars { item_state: self.clone() });
		let writer: Id<SNEPasteboardWriter> = unsafe { msg_send_id![super(writer), init] };

		let info = self.virtual_file_info();

		match info {
			Some(info) => unsafe {
                println!("REMOVE ME ====================== create_item, info: majorVersion {}", majorVersion());

				let provider = SNEForwardingFilePromiseProvider::init(
					SNEForwardingFilePromiseProvider::alloc(),
				);
				provider.setFileType(&NSString::from_str(&info.format));
				provider.setDelegate(Some(&Id::cast(writer.clone())));
				provider.setWritingDelegate(Some(&Id::cast(writer)));
				Id::cast(provider)

                // Id::cast(writer)
			},
			None => unsafe { Id::cast(writer) },
		}
	}

	fn virtual_file_info(self: &Rc<Self>) -> Option<VirtualFileInfo> {
        println!("REMOVE ME ====================== virtual_file_info");
        let data = &self.data_provider.data;
			data.representations.iter().find_map(|item| match item {
				DataRepresentation::VirtualFile { id, format, storage_suggestion: _ } => {
					Some(VirtualFileInfo { id: *id, format: format.clone() })
				}
				_ => None,
			})
	}

	fn writable_types(&self) -> Id<NSArray<NSPasteboardType>> {
        let data = &self.data_provider.data;
        let types: Vec<_> = data
            .representations
            .iter()
            .filter_map(|d| match d {
                DataRepresentation::Simple { format, data: _ } => {
                    Some(NSString::from_str(format))
                }
                DataRepresentation::Lazy { format, id: _ } => {
                    Some(NSString::from_str(format))
                }
                DataRepresentation::VirtualFile { id: _, format, storage_suggestion: _ } => {
                    Some(NSString::from_str(format))
                }
                _ => None,
            })
            .collect();
        println!("REMOVE ME ====================== writable_types, {:?}", &types);
        // Dragging will fail with empty pasteboard. But it is a valid
        // use case in case we have only local data
        if types.is_empty() && self.is_for_dragging {
            NSArray::from_vec(vec![NSString::from_str("dev.nativeshell.placeholder-item")])
        } else {
            NSArray::from_vec(types)
        }
	}

	fn get_lazy_data(
		data_id: DataProviderValueId,
		on_done: Option<Box<dyn FnOnce()>>,
	) -> Arc<ValuePromise> {
		println!("REMOVE ME ====================== get_lazy_data");
		let res = Arc::new(ValuePromise::new());
		let res_clone = res.clone();
		spawn(async move {
			let res = Self::get_lazy_data_async(data_id).await;
			res_clone.set(res);
			if let Some(on_done) = on_done {
				on_done();
			}
		});
		res
	}

	async fn get_lazy_data_async(value_id: DataProviderValueId) -> ValuePromiseResult {
		println!("REMOVE ME ====================== get_lazy_data_async");
		ValuePromiseResult::Ok { value: Value::String("/tmp/test3.txt".to_owned()) }
	}

	fn get_virtual_file(
		virtual_file_id: DataProviderValueId,
		stream_handle: i32,
		on_size_known: Box<dyn Fn(Option<i64>)>,
		on_progress: Box<dyn Fn(f64 /* 0.0 - 1.0 */)>,
		on_done: Box<dyn FnOnce(VirtualFileResult)>,
	) -> Arc<VirtualSessionHandle> {
		println!("REMOVE ME ====================== get_virtual_file");
		Arc::new(VirtualSessionHandle {})
	}

	fn object_for_type(&self, pasteboard_type: &NSPasteboardType) -> Option<Id<NSObject>> {
        let ty = pasteboard_type.to_string();
        let data = &self.data_provider.data;
        for repr in &data.representations {
            match repr {
                DataRepresentation::Simple { format, data } => {
                    if &ty == format {
                        return data.to_objc().ok_log().flatten();
                    }
                }
                DataRepresentation::Lazy { format, id } => {
                    if &ty == format {
                        let promise = Self::get_lazy_data(*id, None);
                        let mut poll_session = PollSession::new();
                        loop {
                            if let Some(result) = promise.try_take() {
                                match result {
                                    ValuePromiseResult::Ok { value } => {
                                        return value.to_objc().ok_log().flatten()
                                    }
                                    ValuePromiseResult::Cancelled => {
                                        return None;
                                    }
                                }
                            }
                            PlatformDataProvider::set_waiting_for_pasteboard_data(true);
                            RunLoop::current()
                                .platform_run_loop
                                .poll_once(&mut poll_session);
                            PlatformDataProvider::set_waiting_for_pasteboard_data(false);
                        }
                    }
                }
                _ => {}
            }
        }
        None
	}

	fn file_promise_file_name_for_type(self: &Rc<Self>, _file_type: &NSString) -> Id<NSString> {
        println!("REMOVE ME ====================== file_promise_file_name_for_type");
        let data = &self.data_provider.data;
        data.suggested_name
            .as_ref()
            .map(|name| NSString::from_str(name))
            .unwrap_or(unsafe { NSString::string() })
	}

	fn progress_for_url(url: &NSURL) -> Id<NSProgress> {
		unsafe {
			let progress = NSProgress::initWithParent_userInfo(NSProgress::alloc(), None, None);
			progress.setKind(Some(NSProgressKindFile));
			progress.setFileURL(Some(url));
			progress.setCancellable(true);
			progress.publish();
			progress
		}
	}

	fn file_promise_do_write(
		self: &Rc<Self>,
		url: &NSURL,
		completion_fn: Box<dyn FnOnce(Option<Id<NSError>>)>,
		info: VirtualFileInfo,
		data_provider: &PlatformDataProvider,
	) {
		let progress = Self::progress_for_url(url);

		let path = path_from_url(url);
        println!("REMOVE ME ====================== file_promise_do_write, path: {:?}", path);
		let file = File::create(&path);
		let file = match file {
			Ok(file) => file,
			Err(err) => {
				let error = to_nserror("super_dnd", 0, &err.to_string());
				completion_fn(Some(error));
				return;
			}
		};
		let descriptor = file.into_raw_fd();
		FILE_PATHS.lock().unwrap().insert(descriptor, path);

		let progress_clone = progress.clone();
		let progress_clone2 = progress.clone();
		let notifier = Self::get_virtual_file(
			info.id,
			descriptor,
			Box::new(|_| {}),
			Box::new(move |cnt| {
				let completed = (cnt * 1000.0).round() as i64;
				unsafe { progress_clone.setCompletedUnitCount(completed) };
			}),
			Box::new(move |result| {
				unsafe { progress_clone2.unpublish() };
				match result {
					VirtualFileResult::Done => completion_fn(None),
					VirtualFileResult::Error { message } => {
						let error = to_nserror("super_dnd", 0, &message);
						completion_fn(Some(error));
					}
					VirtualFileResult::Cancelled => {
						let error = to_nserror("super_dnd", 0, "Cancelled");
						completion_fn(Some(error));
					}
				}
			}),
		);
	}

	fn file_promise_write_to_url(
		self: &Rc<Self>,
		url: &NSURL,
		completion_fn: Box<dyn FnOnce(Option<Id<NSError>>)>,
	) {
		let info = self.virtual_file_info();
		match info {
			Some(info) => {
				self.file_promise_do_write(url, completion_fn, info, &self.data_provider);
			}
			_ => {
				let error = to_nserror("super_dnd", 0, "data not found");
				completion_fn(Some(error));
			}
		}
	}
}

struct Ivars {
	item_state: Rc<ItemState>,
}

declare_class!(
	struct SNEPasteboardWriter;

	unsafe impl ClassType for SNEPasteboardWriter {
		type Super = NSObject;
		type Mutability = mutability::InteriorMutable;
		const NAME: &'static str = "SNEPasteboardWriter";
	}

	impl DeclaredClass for SNEPasteboardWriter {
		type Ivars = Ivars;
	}

	unsafe impl NSObjectProtocol for SNEPasteboardWriter {}

	unsafe impl NSPasteboardWriting for SNEPasteboardWriter {
		#[method_id(writableTypesForPasteboard:)]
		#[allow(non_snake_case)]
		unsafe fn writableTypesForPasteboard(
			&self,
			_pasteboard: &NSPasteboard,
		) -> Id<NSArray<NSPasteboardType>> {
			let ts = self.ivars().item_state.writable_types();
            println!("REMOVE ME ====================== writableTypesForPasteboard, {:?}", &ts);
            ts
		}

		#[method(writingOptionsForType:pasteboard:)]
		#[allow(non_snake_case)]
		unsafe fn writingOptionsForType_pasteboard(
			&self,
			r#_type: &NSPasteboardType,
			_pasteboard: &NSPasteboard,
		) -> NSPasteboardWritingOptions {
            println!("REMOVE ME ====================== writingOptionsForType_pasteboard");
			NSPasteboardWritingOptions::NSPasteboardWritingPromised
		}

		#[method_id(pasteboardPropertyListForType:)]
		#[allow(non_snake_case)]
		unsafe fn pasteboardPropertyListForType(
			&self,
			r#type: &NSPasteboardType,
		) -> Option<Id<AnyObject>> {
            println!("REMOVE ME ====================== pasteboardPropertyListForType, {:?}", &r#type);
			self.ivars().item_state.object_for_type(r#type).map(|v| Id::cast(v))
		}
	}

	unsafe impl NSFilePromiseProviderDelegate for SNEPasteboardWriter {
		#[method_id(filePromiseProvider:fileNameForType:)]
		#[allow(non_snake_case)]
		unsafe fn filePromiseProvider_fileNameForType(
			&self,
			_file_promise_provider: &NSFilePromiseProvider,
			file_type: &NSString,
		) -> Id<NSString> {
            println!("REMOVE ME ====================== filePromiseProvider_fileNameForType");
			self.ivars().item_state.file_promise_file_name_for_type(file_type)
		}

		#[method(filePromiseProvider:writePromiseToURL:completionHandler:)]
		#[allow(non_snake_case)]
		unsafe fn filePromiseProvider_writePromiseToURL_completionHandler(
			&self,
			_file_promise_provider: &NSFilePromiseProvider,
			url: &NSURL,
			completion_handler: &Block<dyn Fn(*mut NSError)>,
		) {
            println!("REMOVE ME ====================== filePromiseProvider_writePromiseToURL_completionHandler");
			let completion_handler =
				RcBlock::<dyn Fn(*mut NSError)>::copy(completion_handler as *const _ as *mut _).unwrap();
			let completion_fn = move |error: Option<Id<NSError>>| {
				let error = match error {
					Some(error) => Id::as_ptr(&error),
					None => std::ptr::null_mut(),
				};
				completion_handler.call((error as *mut _,));
			};
			self.ivars().item_state
				.file_promise_write_to_url(url, Box::new(completion_fn));
		}
	}

	unsafe impl SNEPasteboardWriter {}
);

extern_class!(
	#[derive(PartialEq, Eq, Hash)]
	pub struct SNEForwardingFilePromiseProvider;

	unsafe impl ClassType for SNEForwardingFilePromiseProvider {
		type Super = NSFilePromiseProvider;
		type Mutability = InteriorMutable;
        const NAME: &'static str = "SNEForwardingFilePromiseProvider";
	}
);

extern_methods!(
	unsafe impl SNEForwardingFilePromiseProvider {
		#[allow(non_snake_case)]
		#[method(setWritingDelegate:)]
		pub unsafe fn setWritingDelegate(
			&self,
			delgate: Option<&ProtocolObject<dyn NSPasteboardWriting>>,
		);

		#[method_id(@__retain_semantics Init init)]
		pub unsafe fn init(this: Allocated<Self>) -> Id<Self>;
	}
);
