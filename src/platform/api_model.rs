use std::{
    cell::{Cell, RefCell}, collections::HashMap, ffi::{CStr, OsStr}, fs::File, io::Write, mem::ManuallyDrop, os::unix::{ffi::OsStrExt, prelude::{FromRawFd, IntoRawFd}}, path::PathBuf, rc::{Rc, Weak}, sync::{Arc, Mutex}
};

use block2::{Block, RcBlock};
use irondash_message_channel::{value_darwin::ValueObjcConversion, IntoValue, IsolateId, Late, TryFromValue, Value};
use irondash_run_loop::{platform::PollSession, RunLoop};
use objc2::{
    declare_class, extern_class, extern_methods, msg_send_id,
    mutability::{self, InteriorMutable},
    rc::{Allocated, Id},
    runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
    ClassType, DeclaredClass,
};
use objc2_app_kit::{
    NSFilePromiseProvider, NSFilePromiseProviderDelegate, NSPasteboard, NSPasteboardType,
    NSPasteboardWriting, NSPasteboardWritingOptions,
};
use objc2_foundation::{ns_string, NSArray, NSDictionary, NSError, NSInteger, NSProgress, NSProgressKindFile, NSString, NSURL};
use once_cell::sync::Lazy;

//
// Data Provider
//

pub struct DataProviderHandle;

#[derive(Debug, TryFromValue, IntoValue, Clone, Copy, PartialEq, Hash, Eq)]
pub struct DataProviderValueId(pub i64);

#[derive(Debug, TryFromValue, IntoValue, Clone, Copy, PartialEq, Hash, Eq)]
pub struct DataProviderId(i64);

impl From<i64> for DataProviderId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

#[derive(Debug, TryFromValue, IntoValue, Clone, PartialEq, Eq)]
#[irondash(tag = "type", rename_all = "camelCase")]
pub enum DataRepresentation {
    #[irondash(rename_all = "camelCase")]
    Simple { format: String, data: Value },
    #[irondash(rename_all = "camelCase")]
    Lazy {
        id: DataProviderValueId,
        format: String,
    },
    #[irondash(rename_all = "camelCase")]
    VirtualFile {
        id: DataProviderValueId,
        format: String,
        storage_suggestion: Option<VirtualFileStorage>,
    },
}

impl DataRepresentation {
    pub fn is_virtual_file(&self) -> bool {
        matches!(
            self,
            Self::VirtualFile {
                id: _,
                format: _,
                storage_suggestion: _,
            }
        )
    }
    pub fn format(&self) -> &str {
        match self {
            DataRepresentation::Simple { format, data: _ } => format,
            DataRepresentation::Lazy { id: _, format } => format,
            DataRepresentation::VirtualFile {
                id: _,
                format,
                storage_suggestion: _,
            } => format,
        }
    }
}

#[derive(Debug, TryFromValue, IntoValue, Clone, PartialEq, Eq)]
#[irondash(rename_all = "camelCase")]
pub struct DataProvider {
    pub representations: Vec<DataRepresentation>,
    pub suggested_name: Option<String>,
}

//

#[derive(Debug, TryFromValue, IntoValue, Copy, Clone, PartialEq, Eq)]
#[irondash(rename_all = "camelCase")]
pub enum VirtualFileStorage {
    TemporaryFile,
    Memory,
}
