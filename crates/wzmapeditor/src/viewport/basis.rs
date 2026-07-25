//! Browser-side KTX2/UASTC decode via the Basis Universal transcoder.
//!
//! `basis-universal` is a C++ FFI crate and cannot link on
//! `wasm32-unknown-unknown`, so the web build drives the vendored Binomial
//! `basis_transcoder.wasm` (a separate WebAssembly module) through the JS glue
//! in `js/basis_glue.js`. The transcoder loads asynchronously and is shared
//! across all decodes; [`transcode_ktx2_to_rgba`] is a synchronous call once
//! [`ensure_initialized`] has reported [`is_ready`].

use std::cell::Cell;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen(module = "/js/basis_glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = initBasis, catch)]
    async fn init_basis(js_url: &str, wasm_url: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = transcodeKtx2)]
    fn transcode_ktx2(bytes: &[u8]) -> JsValue;
}

/// Vendored transcoder filenames, copied to the site root by Trunk.
const TRANSCODER_JS: &str = "basis_transcoder.js";
const TRANSCODER_WASM: &str = "basis_transcoder.wasm";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InitState {
    Idle,
    Loading,
    Ready,
    Failed,
}

thread_local! {
    static STATE: Cell<InitState> = const { Cell::new(InitState::Idle) };
}

/// Whether the transcoder has finished loading and can decode KTX2 now.
pub(crate) fn is_ready() -> bool {
    STATE.with(|s| s.get() == InitState::Ready)
}

/// Whether the transcoder gave up loading. HQ terrain is then unavailable, so
/// the loading splash stops waiting on it rather than hanging forever.
pub(crate) fn is_failed() -> bool {
    STATE.with(|s| s.get() == InitState::Failed)
}

/// Start loading the transcoder module; a no-op while a load is in flight or
/// has already succeeded, but a previous failure is retried.
///
/// The transcoder is a ~530 KB WebAssembly module fetched on demand, so this is
/// only called once an HQ terrain pack is present. `ctx` is repainted on
/// completion so the editor re-evaluates HQ availability on the next frame.
///
/// Retrying from `Failed` is safe without a generation guard: that state is
/// only reached after the previous attempt's future has resolved, so no
/// in-flight writer can race the restart. This lets a user "Retry" or a fresh
/// `high.wz` upload recover from a transient fetch/decoder error instead of
/// leaving the transcoder stuck forever.
pub(crate) fn ensure_initialized(ctx: &egui::Context) {
    let should_start = STATE.with(|s| {
        // `Loading` (in flight) and `Ready` are left untouched so a retry never
        // spawns a duplicate load; `Idle` and `Failed` (re)start one.
        let start = matches!(s.get(), InitState::Idle | InitState::Failed);
        if start {
            s.set(InitState::Loading);
        }
        start
    });
    if !should_start {
        return;
    }

    let ctx = ctx.clone();
    spawn_local(async move {
        let next = match init_basis(TRANSCODER_JS, TRANSCODER_WASM).await {
            Ok(_) => {
                log::info!("Basis Universal transcoder ready");
                InitState::Ready
            }
            Err(e) => {
                log::warn!("Failed to load Basis Universal transcoder: {e:?}");
                InitState::Failed
            }
        };
        STATE.with(|s| s.set(next));
        ctx.request_repaint();
    });
}

/// Decode a KTX2/UASTC file to `(width, height, rgba8)`.
///
/// Returns `None` when the transcoder is not yet ready or the file cannot be
/// transcoded, so callers fall back to the PNG path exactly as they do natively
/// on a decode error.
pub(crate) fn transcode_ktx2_to_rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if !is_ready() {
        return None;
    }
    let result = transcode_ktx2(bytes);
    if result.is_null() || result.is_undefined() {
        return None;
    }
    let width = reflect_u32(&result, "width")?;
    let height = reflect_u32(&result, "height")?;
    let data = js_sys::Reflect::get(&result, &JsValue::from_str("data")).ok()?;
    let rgba = js_sys::Uint8Array::new(&data).to_vec();
    Some((width, height, rgba))
}

fn reflect_u32(obj: &JsValue, key: &str) -> Option<u32> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()?
        .as_f64()
        .map(|v| v as u32)
}
