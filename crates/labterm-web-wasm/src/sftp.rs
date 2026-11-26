//! SFTP over the same relay and the same checks as the shell: the host key must match a pin
//! before anything is sent, then the `sftp` subsystem runs on its own SSH connection.
//!
//! Names in a listing come from the server and are not trusted: one that is not a plain file
//! name (empty, `.`, `..`, or with a `/`, `\` or NUL in it) is left out, so it can never become
//! part of a path on this side.

use std::rc::Rc;

use js_sys::{Function, Promise, Uint8Array};
use russh::client;
use russh_sftp::client::SftpSession;
use russh_sftp::client::fs::Metadata;
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{Handler, Options, establish, to_js, transport};

/// Chunks handed to the page while downloading.
const CHUNK: usize = 256 * 1024;

#[derive(Serialize)]
pub struct Entry {
    name: String,
    path: String,
    kind: &'static str,
    size: Option<u64>,
    mode: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
    user: Option<String>,
    group: Option<String>,
    mtime: Option<u32>,
    atime: Option<u32>,
    link_target: Option<String>,
    target_kind: Option<&'static str>,
}

fn kind(t: FileType) -> &'static str {
    match t {
        FileType::Dir => "dir",
        FileType::File => "file",
        FileType::Symlink => "symlink",
        FileType::Other => "other",
    }
}

/// A plain file name, safe to join onto a path on either side.
pub fn safe_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

/// `dir` + `name` with one `/` between them.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn entry(name: String, path: String, m: &Metadata) -> Entry {
    Entry {
        name,
        path,
        kind: kind(m.file_type()),
        size: m.size,
        // Permission bits only; the type is in `kind`.
        mode: m.permissions.map(|p| p & 0o7777),
        uid: m.uid,
        gid: m.gid,
        user: m.user.clone(),
        group: m.group.clone(),
        mtime: m.mtime,
        atime: m.atime,
        link_target: None,
        target_kind: None,
    }
}

fn err(e: impl std::fmt::Display) -> JsValue {
    let e = js_sys::Error::new(&e.to_string());
    let _ = js_sys::Reflect::set(&e, &"kind".into(), &"sftp".into());
    e.into()
}

struct Inner {
    sftp: SftpSession,
    // Keeps the SSH connection open for as long as the SFTP session lives.
    _handle: client::Handle<Handler>,
}

/// An open SFTP session. Every method returns a promise.
#[wasm_bindgen]
pub struct Sftp {
    inner: Rc<Inner>,
}

fn run<F, T>(inner: &Rc<Inner>, f: impl FnOnce(Rc<Inner>) -> F) -> Promise
where
    F: std::future::Future<Output = Result<T, JsValue>> + 'static,
    T: Serialize,
{
    let fut = f(inner.clone());
    future_to_promise(async move {
        let v = fut.await?;
        to_js(&v).map_err(err)
    })
}

#[wasm_bindgen]
impl Sftp {
    /// The directory the server starts in (usually the user's home).
    pub fn home(&self) -> Promise {
        run(&self.inner, |i| async move {
            i.sftp.canonicalize(".").await.map_err(err)
        })
    }

    /// The entries of `path`, links resolved one level for their target's kind.
    pub fn list(&self, path: String) -> Promise {
        run(&self.inner, |i| async move {
            let dir = i.sftp.canonicalize(path).await.map_err(err)?;
            let mut out = Vec::new();
            for e in i.sftp.read_dir(dir.clone()).await.map_err(err)? {
                let name = e.file_name();
                if !safe_name(&name) {
                    continue;
                }
                let path = join(&dir, &name);
                let mut item = entry(name, path.clone(), &e.metadata());
                if item.kind == "symlink" {
                    item.link_target = i.sftp.read_link(path.clone()).await.ok();
                    item.target_kind = i
                        .sftp
                        .metadata(path)
                        .await
                        .ok()
                        .map(|m| kind(m.file_type()));
                }
                out.push(item);
            }
            Ok((dir, out))
        })
    }

    pub fn stat(&self, path: String) -> Promise {
        run(&self.inner, |i| async move {
            let m = i.sftp.symlink_metadata(path.clone()).await.map_err(err)?;
            let name = path.rsplit('/').next().unwrap_or_default().to_string();
            Ok(entry(name, path, &m))
        })
    }

    pub fn mkdir(&self, path: String) -> Promise {
        run(&self.inner, |i| async move {
            i.sftp.create_dir(path).await.map_err(err)
        })
    }

    pub fn rename(&self, from: String, to: String) -> Promise {
        run(&self.inner, |i| async move {
            i.sftp.rename(from, to).await.map_err(err)
        })
    }

    #[wasm_bindgen(js_name = removeFile)]
    pub fn remove_file(&self, path: String) -> Promise {
        run(&self.inner, |i| async move {
            i.sftp.remove_file(path).await.map_err(err)
        })
    }

    #[wasm_bindgen(js_name = removeDir)]
    pub fn remove_dir(&self, path: String) -> Promise {
        run(&self.inner, |i| async move {
            i.sftp.remove_dir(path).await.map_err(err)
        })
    }

    pub fn chmod(&self, path: String, mode: u32) -> Promise {
        run(&self.inner, |i| async move {
            let attrs = FileAttributes {
                permissions: Some(mode & 0o7777),
                ..FileAttributes::empty()
            };
            i.sftp.set_metadata(path, attrs).await.map_err(err)
        })
    }

    /// Reads `path` and hands it to `on_chunk(Uint8Array)` piece by piece; resolves with the size.
    pub fn download(&self, path: String, on_chunk: Function) -> Promise {
        run(&self.inner, |i| async move {
            let mut f = i.sftp.open(path).await.map_err(err)?;
            let mut buf = vec![0u8; CHUNK];
            let mut total: u64 = 0;
            loop {
                let n = f.read(&mut buf).await.map_err(err)?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                let chunk = Uint8Array::from(&buf[..n]);
                on_chunk.call1(&JsValue::NULL, &chunk)?;
            }
            Ok(total)
        })
    }

    /// Writes one piece of an upload: the first (`append` false) creates or truncates `path`.
    pub fn upload(&self, path: String, data: Vec<u8>, append: bool) -> Promise {
        run(&self.inner, |i| async move {
            let flags = if append {
                OpenFlags::WRITE | OpenFlags::APPEND
            } else {
                OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE
            };
            let mut f = i.sftp.open_with_flags(path, flags).await.map_err(err)?;
            f.write_all(&data).await.map_err(err)?;
            f.shutdown().await.map_err(err)?;
            Ok(data.len())
        })
    }

    pub fn close(&self) -> Promise {
        run(
            &self.inner,
            |i| async move { i.sftp.close().await.map_err(err) },
        )
    }
}

/// Opens an SFTP session: the same options as `connect` (relay URL, host, pins, auth).
#[wasm_bindgen(js_name = connectSftp)]
pub async fn connect_sftp(
    options: JsValue,
    on_event: Function,
    prompt: Option<Function>,
) -> Result<Sftp, JsValue> {
    let o: Options =
        serde_wasm_bindgen::from_value(options).map_err(|e| transport(format!("options: {e}")))?;
    let handle = establish(&o, &on_event, prompt.as_ref()).await?;
    let ch = handle.channel_open_session().await.map_err(transport)?;
    ch.request_subsystem(true, "sftp")
        .await
        .map_err(transport)?;
    let sftp = SftpSession::new(ch.into_stream())
        .await
        .map_err(|e| transport(format!("sftp: {e}")))?;
    Ok(Sftp {
        inner: Rc::new(Inner {
            sftp,
            _handle: handle,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_the_server_stay_names() {
        for bad in ["", ".", "..", "../x", "a/b", "..\\x", "a\0b"] {
            assert!(!safe_name(bad), "{bad:?}");
        }
        for good in ["file.txt", "..hidden", "a b", "ü.txt"] {
            assert!(safe_name(good), "{good:?}");
        }
        assert_eq!(join("/home/u", "x"), "/home/u/x");
        assert_eq!(join("/", "x"), "/x");
    }
}
