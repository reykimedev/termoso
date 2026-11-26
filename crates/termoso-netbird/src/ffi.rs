//! The Go side (`go/main.go`), loaded with `dlopen` on first use so the Go
//! runtime only starts when a host actually uses NetBird.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::OnceLock;

/// File name of the Go library (`go build -buildmode=c-shared`).
pub const LIBRARY: &str = "libtermoso_netbird.so";

type StartFn = unsafe extern "C" fn(*const c_char, *mut usize) -> *mut c_char;
type DialFn = unsafe extern "C" fn(usize, *const c_char, c_int, c_int, *mut *mut c_char) -> c_int;
type StopFn = unsafe extern "C" fn(usize);
type FreeFn = unsafe extern "C" fn(*mut c_char);

pub(crate) struct Library {
    start: StartFn,
    dial: DialFn,
    stop: StopFn,
    free: FreeFn,
}

// The functions are plain C entry points of a library that is never
// unloaded (a Go runtime cannot be).
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

static LOADED: OnceLock<std::result::Result<Library, String>> = OnceLock::new();

/// Load the library once; later calls return the same result. `path`
/// overrides where to look (tests); otherwise the platform's library search
/// path is used (on Android: the app's native library directory).
pub(crate) fn library(path: Option<&str>) -> std::result::Result<&'static Library, String> {
    LOADED
        .get_or_init(|| unsafe { open(path.unwrap_or(LIBRARY)) })
        .as_ref()
        .map_err(Clone::clone)
}

unsafe fn open(path: &str) -> std::result::Result<Library, String> {
    let c_path = CString::new(path).map_err(|e| e.to_string())?;
    // RTLD_NODELETE: the Go runtime must never be unmapped.
    let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_NODELETE) };
    if handle.is_null() {
        let why = unsafe { libc::dlerror() };
        let why = if why.is_null() {
            "unknown error".to_string()
        } else {
            unsafe { CStr::from_ptr(why) }
                .to_string_lossy()
                .into_owned()
        };
        return Err(format!("NetBird is not available in this build ({why})"));
    }
    unsafe fn sym(handle: *mut c_void, name: &str) -> std::result::Result<*mut c_void, String> {
        let c = CString::new(name).expect("symbol name");
        let p = unsafe { libc::dlsym(handle, c.as_ptr()) };
        if p.is_null() {
            Err(format!("NetBird library lacks {name}"))
        } else {
            Ok(p)
        }
    }
    unsafe {
        Ok(Library {
            start: std::mem::transmute::<*mut c_void, StartFn>(sym(handle, "tnb_start")?),
            dial: std::mem::transmute::<*mut c_void, DialFn>(sym(handle, "tnb_dial")?),
            stop: std::mem::transmute::<*mut c_void, StopFn>(sym(handle, "tnb_stop")?),
            free: std::mem::transmute::<*mut c_void, FreeFn>(sym(handle, "tnb_free")?),
        })
    }
}

impl Library {
    fn take(&self, s: *mut c_char) -> String {
        if s.is_null() {
            return String::new();
        }
        let out = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
        unsafe { (self.free)(s) };
        out
    }

    /// Start a node; blocks until it is up. Returns the handle and the
    /// JSON result (`{"private_key":…,"registered":…}`).
    pub(crate) fn start(&self, options_json: &str) -> std::result::Result<(usize, String), String> {
        let c = CString::new(options_json).map_err(|e| e.to_string())?;
        let mut handle: usize = 0;
        let out = unsafe { (self.start)(c.as_ptr(), &mut handle) };
        let text = self.take(out);
        if handle == 0 {
            Err(text)
        } else {
            Ok((handle, text))
        }
    }

    /// Open `host:port` in the node's network; blocks until connected.
    pub(crate) fn dial(
        &self,
        handle: usize,
        host: &str,
        port: u16,
        timeout_ms: u32,
    ) -> std::result::Result<OwnedFd, String> {
        let c = CString::new(host).map_err(|e| e.to_string())?;
        let mut err: *mut c_char = std::ptr::null_mut();
        let fd = unsafe {
            (self.dial)(
                handle,
                c.as_ptr(),
                c_int::from(port),
                timeout_ms.min(c_int::MAX as u32) as c_int,
                &mut err,
            )
        };
        if fd < 0 {
            return Err(self.take(err));
        }
        // The Go side handed us this end of the socket pair and forgot it.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    /// Stop a node and free its handle.
    pub(crate) fn stop(&self, handle: usize) {
        unsafe { (self.stop)(handle) }
    }
}
