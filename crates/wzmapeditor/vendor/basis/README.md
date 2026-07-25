# Vendored Basis Universal transcoder

`basis_transcoder.js` and `basis_transcoder.wasm` are the Binomial Basis
Universal transcoder, used by the web build to decode the uploaded `high.wz`
KTX2/UASTC terrain textures in the browser (the native build links the
`basis-universal` C++ crate instead, which cannot target
`wasm32-unknown-unknown`).

- Upstream: <https://github.com/BinomialLLC/basis_universal> (`webgl/transcoder`)
  at tag `v1_50_0_2`, the same version the three.js distribution ships.
- License: Apache-2.0 — compatible with this project's GPL-2.0-or-later.

The build is compiled with KTX2 Zstandard support, so `KTX2File` decompresses
zstd-supercompressed levels internally. Loaded on demand via `js/basis_glue.js`.

## Rebuilding

This is a local build rather than the prebuilt three.js artifact, because the
prebuilt one cannot run under the deployment's Content-Security-Policy: embind
generates its call wrappers with `new Function()`, which needs `unsafe-eval`.
`-sDYNAMIC_EXECUTION=0` makes embind use closure-based wrappers instead, so the
transcoder only needs `wasm-unsafe-eval`.

```sh
git clone --depth 1 --branch v1_50_0_2 \
    https://github.com/BinomialLLC/basis_universal.git
cd basis_universal/webgl/transcoder
```

Apply three changes to `CMakeLists.txt`, then `emcmake cmake . && emmake make`:

- `CMAKE_CXX_STANDARD` 11 → 17, as current embind headers require C++17.
- Append `-s EXPORTED_RUNTIME_METHODS=['HEAP8']` to `LINK_FLAGS`.
  `basis_wrappers.cpp` reads `Module.HEAP8` to copy buffers in and out of the
  wasm heap; emscripten no longer exports it by default, and without it every
  such call sees `undefined`.
- Append `-s DYNAMIC_EXECUTION=0` to `LINK_FLAGS`.

The first two are unrelated to CSP and are needed to build v1.50 with a current
emscripten at all; upstream `master` already carries both.

Transcoding is unaffected by these flags: the emitted `.wasm` is byte-identical
with and without `DYNAMIC_EXECUTION=0`, and the decoded RGBA output matches the
previous prebuilt artifact byte-for-byte across every mip level.
