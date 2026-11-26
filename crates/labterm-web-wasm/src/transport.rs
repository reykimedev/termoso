//! The relay's WebSocket as a byte stream for russh.
//!
//! russh wants a `Send` stream; browser objects are not `Send`. The socket lives in local tasks
//! and talks to russh through an in-memory duplex pipe, which is.

use futures::StreamExt;
use futures::channel::mpsc;
use js_sys::{ArrayBuffer, Uint8Array};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

fn err(msg: impl Into<String>) -> JsValue {
    super::transport(msg.into())
}

pub async fn websocket(url: &str) -> Result<tokio::io::DuplexStream, JsValue> {
    let ws = WebSocket::new(url).map_err(|_| err("cannot open the relay connection"))?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    let (open_tx, mut open_rx) = mpsc::unbounded::<Result<(), String>>();
    let (in_tx, mut in_rx) = mpsc::unbounded::<Option<Vec<u8>>>();
    let t = open_tx.clone();
    let onopen = Closure::<dyn FnMut()>::new(move || {
        let _ = t.unbounded_send(Ok(()));
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();
    let t = in_tx.clone();
    let onmsg = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        if let Ok(buf) = e.data().dyn_into::<ArrayBuffer>() {
            let _ = t.unbounded_send(Some(Uint8Array::new(&buf).to_vec()));
        }
    });
    ws.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));
    onmsg.forget();
    let (t, o) = (in_tx.clone(), open_tx.clone());
    let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
        // Before open, the relay refused the upgrade (ticket, allowlist, limits).
        let reason = e.reason();
        let _ = o.unbounded_send(Err(if reason.is_empty() {
            "the relay refused the connection".into()
        } else {
            reason
        }));
        let _ = t.unbounded_send(None);
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    match open_rx.next().await {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(err(e)),
        None => return Err(err("the relay connection closed")),
    }

    let (russh_side, bridge) = tokio::io::duplex(256 * 1024);
    let (mut br, mut bw) = tokio::io::split(bridge);
    wasm_bindgen_futures::spawn_local(async move {
        while let Some(Some(chunk)) = in_rx.next().await {
            if bw.write_all(&chunk).await.is_err() {
                break;
            }
        }
        let _ = bw.shutdown().await;
    });
    wasm_bindgen_futures::spawn_local(async move {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match br.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws.send_with_u8_array(&buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ws.close();
    });
    Ok(russh_side)
}
