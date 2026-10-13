# Performance and GPU setup

## Design

Iced renders native widgets through wgpu. Network requests use Tokio/reqwest;
image decoding and neural inference run off the UI thread. Requests, decoding,
and enhancement have separate concurrency limits (4, 2, and 1). Library covers
are downsampled to 240×340. Chapter lists create at most 100 rows at a time.
Webtoon layout uses spacers for offscreen pages. Decoded originals share their
RGBA allocation with the renderer and the enhancement task.

The decoded image cache defaults to 192 MiB. It keeps the current spread, so this
is a soft cache budget, not a process memory ceiling. It excludes GPU allocations,
in-flight work, cover images, fonts, runtime libraries, and other application
state. The temporary disk cache defaults to 1 GiB and is pruned periodically;
pinned chapter downloads are exempt. There is no retention policy for old server
profiles yet.

## Measured on the development machine

Arch Linux, Hyprland/Wayland, Ryzen 7 5700X, RTX 4080 Super, 32 GB RAM, release build.
These are development spot checks, not a statistical benchmark or a comparison
with Tsumiru. Driver caches, loaded libraries, compositor state, page dimensions,
and other running programs affect the results.

| Check | Observation |
| --- | --- |
| Fast 2× UpConv7, original 600×900 fixture → 1200×1800 | 350.6 ms cold; 68.6 and 67.5 ms warm |
| Optional CUnet 2×, same fixture | 1555 ms cold; 512.6 and 509.3 ms warm |
| Native connection window, initial development smoke | Mapped in 643 ms; 181.6 MiB RSS; 80.0 MiB PSS; XWayland false |
| Offline fixture library, later feature build | 532 ms mapped; 223.7 MiB RSS; 111.8 MiB PSS |
| Offline fixture paged reader | 322 ms mapped; 275.6 MiB RSS; 147.8 MiB PSS |
| Offline fixture webtoon | 325 ms mapped; 303.6 MiB RSS; 175.9 MiB PSS |

The fixture UI runs used a 1249×1394 tiled window and sampled memory shortly after
rendering. All reported native windows had XWayland disabled. UI test images are
original procedural fixtures rather than downloaded manga.

Neural timings include model/session initialization on the cold pass, CPU tile
preparation, transfers, inference, and output assembly. They exclude file decode,
PNG encoding, network fetch, and display presentation. Upscaling is asynchronous
page enhancement, not a promise of 60 fps inference. Tall webtoon images take more
time and memory. A single enhanced output is limited to 256 MiB.

A final smoke check waited until the native reader displayed its enhanced page.
It measured 945.1 MiB RSS and 811.1 MiB PSS, with a 323 ms window-map time. A plain
reader run with the same fixture measured 224.3 MiB RSS / 130.7 MiB PSS. CUDA/cuDNN
and inference initialization have a substantial memory cost beyond the page cache.
The plain-reader run overlapped compilation, so its startup time is not used for
an uncontended startup comparison. These are point-in-time readings, not peak
memory measurements. GPU memory is separate from these host-memory figures.

The expanded catalog's Settings window was also checked at 1240×900 and 820×600.
It sampled approximately 185 MiB RSS / 127 MiB PSS, with XWayland disabled. An
IllustrationJaNai Denoise FP16 reader smoke waited for an actual enhanced page and
sampled 1347 MiB RSS / 1220 MiB PSS. These are separate point-in-time readings,
with different loaded inference kernels and caches, not peak-memory comparisons.
The neural runtime remains a substantial optional memory cost.

## Reproduce neural timing

```sh
./scripts/run.sh --upscale-check models/fast-2x.json input.png output.png
```

The output must not already exist. The command performs three passes and saves the
third result. Models use float32 or float16 NCHW RGB data with values in [0, 1]. The bundled
model manifest records the SHA-256, scale, tile size, and input padding.

The expanded catalog has 11 presets with 17 validated exports. See
[the current model table](upscaling-research.md) for their descriptions and timing
ranges. Those checks used the same 600×900 fixture and include 2× and 4× outputs.

## Tested runtime

The development checkout uses the official [ONNX Runtime 1.24.2 CUDA 13 release](https://github.com/microsoft/onnxruntime/releases/tag/v1.24.2)
and cuDNN 9.27.0.42. Runtime archive:
`onnxruntime-linux-x64-gpu_cuda13-1.24.2.tgz`, SHA-256:
`75128942ac9b22fb5001f1e1e041b473793958db016cd124a2ad981fe273ae12`.
The archive extracts to `onnxruntime-linux-x64-gpu-1.24.2`.

Set `ORT_DYLIB_PATH` to its `lib/libonnxruntime.so`. Keep the other provider shared
libraries in that directory. Put CUDA and cuDNN library directories on
`LD_LIBRARY_PATH` if they are not already registered with the system loader.
See the upstream [CUDA execution provider requirements](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)
when choosing a different runtime build. CUDA major versions and cuDNN major
versions must match the runtime's requirements.

The loader checks the library and API version before initializing ONNX Runtime.
CUDA registration is strict: an unavailable GPU reports an error instead of
silently running this neural model on the CPU. The normal reader remains usable.
