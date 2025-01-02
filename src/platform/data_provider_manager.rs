use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    os::raw::c_void,
    rc::{Rc, Weak},
    slice,
    sync::Arc,
};

use async_trait::async_trait;
use irondash_message_channel::{
    AsyncMethodHandler, AsyncMethodInvoker, IntoPlatformResult, IntoValue, IsolateId, Late,
    MethodCall, PlatformError, PlatformResult, RegisteredAsyncMethodHandler, TryFromValue, Value,
};
use irondash_run_loop::spawn;

use super::{
    api_model::{DataProvider, DataProviderId, DataProviderValueId}, data_provider::PlatformDataProvider, error::{NativeExtensionsError, NativeExtensionsResult}, log::OkLog, value_promise::{ValuePromise, ValuePromiseResult, ValuePromiseSetCancel}
};

pub enum VirtualFileResult {
    Done,
    Error { message: String },
    Cancelled,
}

/// Keeps the virtual session alive
pub struct VirtualSessionHandle;


#[async_trait(?Send)]
pub trait PlatformDataProviderDelegate {
    fn get_lazy_data(
        &self,
        data_id: DataProviderValueId,
        on_done: Option<Box<dyn FnOnce()>>,
    ) -> Arc<ValuePromise>;

    async fn get_lazy_data_async(
        &self,
        data_id: DataProviderValueId,
    ) -> ValuePromiseResult;

    fn get_virtual_file(
        &self,
        virtual_file_id: DataProviderValueId,
        stream_handle: i32,
        on_size_known: Box<dyn Fn(Option<i64>)>,
        on_progress: Box<dyn Fn(f64 /* 0.0 - 1.0 */)>,
        on_done: Box<dyn FnOnce(VirtualFileResult)>,
    ) -> Arc<VirtualSessionHandle>;
}

pub struct DataProviderManager {
    weak_self: Late<Weak<Self>>,
    invoker: Late<AsyncMethodInvoker>,
    next_id: Cell<i64>,
    providers: RefCell<HashMap<DataProviderId, DataProviderEntry>>,
    virtual_sessions: RefCell<HashMap<VirtualSessionId, VirtualFileSession>>,
}

pub trait GetDataProviderManager {
    fn data_provider_manager(&self) -> Rc<DataProviderManager>;
}

// impl GetDataProviderManager for Context {
//     fn data_provider_manager(&self) -> Rc<DataProviderManager> {
//         self.get_attachment(DataProviderManager::new).handler()
//     }
// }

struct DataProviderEntry;

#[derive(Debug, TryFromValue, IntoValue, Clone, Copy, PartialEq, Hash, Eq)]
struct VirtualSessionId(i64);

impl From<i64> for VirtualSessionId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

struct VirtualFileSession {
    size_known: Cell<bool>,
    on_size_known: Box<dyn Fn(Option<i64>)>,
    on_progress: Box<dyn Fn(f64 /* 0.0 - 1.0 */)>,
    on_done: Box<dyn FnOnce(VirtualFileResult)>,
}

impl DataProviderManager {

    fn virtual_file_update_progress(
        &self,
        progress: VirtualFileUpdateProgress,
    ) -> NativeExtensionsResult<()> {
        let sessions = self.virtual_sessions.borrow();
        let session = sessions
            .get(&progress.session_id)
            .ok_or(NativeExtensionsError::VirtualFileSessionNotFound)?;
        (session.on_progress)(progress.progress);
        Ok(())
    }

    fn virtual_file_size_known(
        &self,
        size_known: VirtualFileSizeKnown,
    ) -> NativeExtensionsResult<()> {
        let sessions = self.virtual_sessions.borrow();
        let session = sessions
            .get(&size_known.session_id)
            .ok_or(NativeExtensionsError::VirtualFileSessionNotFound)?;
        session.size_known.replace(true);
        (session.on_size_known)(Some(size_known.file_size));
        Ok(())
    }

    fn virtual_file_complete(&self, complete: VirtualFileComplete) -> NativeExtensionsResult<()> {
        let session = self
            .virtual_sessions
            .borrow_mut()
            .remove(&complete.session_id)
            .ok_or(NativeExtensionsError::VirtualFileSessionNotFound)?;
        if !session.size_known.get() {
            (session.on_size_known)(None);
        }
        (session.on_done)(VirtualFileResult::Done);
        Ok(())
    }

    fn virtual_file_error(&self, error: VirtualFileError) -> NativeExtensionsResult<()> {
        let session = self
            .virtual_sessions
            .borrow_mut()
            .remove(&error.session_id)
            .ok_or(NativeExtensionsError::VirtualFileSessionNotFound)?;
        if !session.size_known.get() {
            (session.on_size_known)(None);
        }
        (session.on_done)(VirtualFileResult::Error {
            message: error.error_message,
        });
        Ok(())
    }

    fn virtual_file_cancel(&self, complete: VirtualFileCancel) -> NativeExtensionsResult<()> {
        let session = self
            .virtual_sessions
            .borrow_mut()
            .remove(&complete.session_id)
            .ok_or(NativeExtensionsError::VirtualFileSessionNotFound)?;
        if !session.size_known.get() {
            (session.on_size_known)(None);
        }
        (session.on_done)(VirtualFileResult::Cancelled);
        Ok(())
    }
}

#[async_trait(?Send)]
impl PlatformDataProviderDelegate for DataProviderManager {
    fn get_lazy_data(
        &self,
        data_id: DataProviderValueId,
        on_done: Option<Box<dyn FnOnce()>>,
    ) -> Arc<ValuePromise> {
        let res = Arc::new(ValuePromise::new());
        let res_clone = res.clone();
        let weak_self = self.weak_self.clone();
        spawn(async move {
            let this = weak_self.upgrade();
            if let Some(this) = this {
                let res = this.get_lazy_data_async(data_id).await;
                res_clone.set(res);
                if let Some(on_done) = on_done {
                    on_done();
                }
            } else {
                res_clone.cancel();
            }
        });
        res
    }

    async fn get_lazy_data_async(
        &self,
        value_id: DataProviderValueId,
    ) -> ValuePromiseResult {
        ValuePromiseResult::Ok { value: Value::String("test1".to_owned()) }
    }

    fn get_virtual_file(
        &self,
        virtual_file_id: DataProviderValueId,
        stream_handle: i32,
        on_size_known: Box<dyn Fn(Option<i64>)>,
        on_progress: Box<dyn Fn(f64 /* 0.0 - 1.0 */)>,
        on_done: Box<dyn FnOnce(VirtualFileResult)>,
    ) -> Arc<VirtualSessionHandle> {
        Arc::new(VirtualSessionHandle{})
    }
}

#[derive(Debug, TryFromValue)]
#[irondash(rename_all = "camelCase")]
struct VirtualFileUpdateProgress {
    session_id: VirtualSessionId,
    progress: f64,
}

#[derive(Debug, TryFromValue)]
#[irondash(rename_all = "camelCase")]
struct VirtualFileSizeKnown {
    session_id: VirtualSessionId,
    file_size: i64,
}

#[derive(Debug, TryFromValue)]
#[irondash(rename_all = "camelCase")]
struct VirtualFileComplete {
    session_id: VirtualSessionId,
}

#[derive(Debug, TryFromValue)]
#[irondash(rename_all = "camelCase")]
struct VirtualFileCancel {
    session_id: VirtualSessionId,
}

#[derive(Debug, TryFromValue)]
#[irondash(rename_all = "camelCase")]
struct VirtualFileError {
    session_id: VirtualSessionId,
    error_message: String,
}
// FFI
