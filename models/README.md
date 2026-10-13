The bundled fast 2× model is nagadomi's MIT-licensed waifu2x UpConv7 art model,
converted from the original published JSON weights with `scripts/convert-upconv.py`.
Its ONNX file is 2.2 MB. The original six leaky-ReLU convolutions, transposed
convolution, and final clamp are preserved. No denoising is applied.

Original weights:
https://github.com/nagadomi/waifu2x/blob/master/models/upconv_7/art/scale2.0x_model.json

Original architecture:
https://github.com/nagadomi/waifu2x/blob/master/lib/srcnn.lua

Original weights SHA-256:
`8e5e66b0826607d29c038d5b8399b71fe9cc23950eb5ec57be5ab1f865f1ca31`

The manifest records the converted model's SHA-256. Conversion used ONNX 1.23.2,
opset 17, and IR version 9. See LICENSE.waifu2x for redistribution terms.

## Optional model catalog

`catalog.json` embeds 11 presets in the native app. It records descriptions,
license and source links, tile parameters, and every installed artifact's SHA-256.
Optional weights are downloaded into the configured model directory; they are
not embedded in the executable or committed to this repository.

- CUnet and SwinUNet use nagadomi's `waifu2x_onnx_models_20250502.zip`.
- MangaJaNai V1 uses haesslerian's community ONNX exports, pinned to Hugging Face
  revision `2f67ff30209f2e0c75b7c55e6f3ffc3adbc960f7`. All seven 2× height presets
  are installed together. They retain the creator's CC-BY-NC-4.0 license.
- IllustrationJaNai V3 uses the creator's official ONNX release archives. The
  selected Denoise and Detail variants are compact SPAN S 2×, with FP16 tensors.
- Real-ESRGAN and Anime4K use rigaya's `20260808` ONNX export release. Anime4K is
  the CNN S upscaler exported from the official shader; it does not include the
  complete mpv shader chain. Real-ESRGAN uses FP32 anime 6B and animevideov3 4×.
- Original AnimeSharp uses Kim2091's FP32 opset-16 export at revision
  `7696d95ced82b0c1f2a41f6ac73336133f0a90e1`, under CC-BY-NC-SA-4.0.

The installer retrieves known compressed byte ranges from release ZIPs rather
than downloading every model in each archive. It requires HTTP 206 and the exact
range, bounds the expanded payload, then checks the complete uncompressed file's
hash before publishing. Direct Hugging Face downloads also have pinned hashes.
Upstream replacements fail verification. Each install writes source, checksum,
and license files next to its manifest. A cancelled retry reuses verified weights.

To install the full catalog into a chosen directory:

```sh
./scripts/run.sh --install-model all .local/models
```

To check every actual export, including all MangaJaNai variants:

```sh
python3 scripts/check-upscalers.py .local/models input.png /tmp/suwa-model-checks
```

The output directory should be new. The command uses the release reader's CUDA
path and writes timing results and enhanced images. No Python inference runtime
is used by the app. See `licenses/` and `docs/upscaling-research.md` for attribution
and model notes.
