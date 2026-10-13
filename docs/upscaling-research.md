# Anime and manga upscalers in Suwa

Integrated and checked 2026-10-13. Settings → Upscaling offers 11 presets across
all the researched families. Fast is bundled; the other weights are optional,
checksum-verified downloads. All 17 actual ONNX exports, including the seven
MangaJaNai variants, passed the native CUDA pipeline on the RTX 4080 Super.

## Presets and descriptions

| Preset | Intended use | Warm time, 600×900 synthetic page |
| --- | --- | --- |
| waifu2x · Fast 2× | UpConv7 art: a small, fast default for clean line art and everyday reading. Upscales without denoising. | 81–92 ms |
| waifu2x · CUnet 2× | A larger convolutional model for anime illustrations and line art. Upscales without denoising; slower than Fast. | 194–200 ms |
| waifu2x · SwinUNet Art 2× | A newer transformer model for digital art, tuned for pixel fidelity. A conservative option for clean color pages and line art. | 404–415 ms |
| waifu2x · SwinUNet Scan 2× | The art_scan GAN variant for printed manga scans and captured artwork. Can alter small details, lettering and screentones. | 581–601 ms |
| MangaJaNai · Auto 2× | V1 ESRGAN restoration for black-and-white digital manga: JPEG artifacts, moiré and damaged halftones. Selects the nearest source-height model from 1200p through 2048p. | 920–1216 ms |
| IllustrationJaNai · Denoise 2× | V3 denoise, compact SPAN S: cleans JPEG artifacts and noise in color manga pages, covers and digital illustrations. | 116–194 ms |
| IllustrationJaNai · Detail 2× | V3 detail, compact SPAN S: restores texture and line detail in color manga pages, covers and illustrations. Can invent or change fine detail. | 108–191 ms |
| Real-ESRGAN · Anime 6B 4× | x4plus_anime_6B: an anime illustration model for degraded color artwork. Restores stronger edges and textures, with a higher inference cost. | 1066–1559 ms |
| Real-ESRGAN · Anime Video 4× | The compact animevideov3 model, trained on animation frames. A faster Real-ESRGAN option for color art; compare small text and screentones. | 208–284 ms |
| Anime4K · CNN S 2× | A very small CNN extracted from the official Anime4K upscale shader. Adds crisp edges to clean anime art. This preset runs the CNN through CUDA; it is not the full mpv shader chain. | 43–66 ms |
| AnimeSharp · 4× | The original 4× AnimeSharp ESRGAN interpolation of UltraSharp and TextSharp. Emphasizes crisp anime edges and lettering; may oversharpen or alter thin lines. | 2804–3101 ms |

The 2× models produce 1200×1800; the 4× models produce 2400×3600. This is not a
comparison at equal output size, a quality ranking, or a guarantee for manga.
Timings include tile preparation, transfers, inference and output assembly. Cold
session initialization took approximately 0.5–6 seconds in these runs. Compilation
overlapped some checks, so the range includes contention. Input decode, downloads,
PNG encoding and display presentation are excluded. Tall webtoons cost more.

The native picker shows the selected preset's description, intended material,
license, installed status or download size, and a source link. Installation is
asynchronous and cancellable. Model selection persists, and switching clears
old enhanced reader pages. Only one model session is retained. Originals remain
visible while enhancement runs; an enhanced output is limited to 256 MiB.

## Exact variants and sources

waifu2x Fast uses the original UpConv7 art 2× model. CUnet and SwinUNet use the
creator's 20250502 ONNX release. Art is the fidelity-focused preset; art_scan is
the GAN-trained scan preset. None of these selected presets applies a denoising
level. [Creator's descriptions](https://github.com/nagadomi/nunif/blob/master/waifu2x/docs/gui_ja.md),
[official exports](https://github.com/nagadomi/nunif/releases/tag/0.0.0).

MangaJaNai Auto installs the seven V1 ESRGAN 2× models: 1200p, 1300p, 1400p, 1500p,
1600p, 1920p and 2048p. Suwa selects the nearest original page height and replaces
the session when that variant changes. This preset is for black-and-white pages;
it does not automatically detect color or switch to IllustrationJaNai. Very tall
strips select 2048p, which is not a claim that these page-trained models suit
webtoons. The models restore JPEG, moiré and halftone artifacts and can reconstruct
details that differ from the source. We use a pinned community ONNX export, under
the upstream CC-BY-NC-4.0 license. [Creator's project](https://github.com/the-database/MangaJaNai),
[export repository](https://huggingface.co/haesslerian/MangaJaNai_V1_ONNX).

IllustrationJaNai Denoise and Detail use the creator's V3 SPAN S 2× FP16 exports.
These are the compact variants for lightly degraded color pages and illustrations.
Denoise cleans halftone noise; Detail retains or generates texture and can add
noise. The larger FDAT, DAT2 and HAT variants are not included in these presets.
[Official model release](https://github.com/the-database/MangaJaNai/releases/tag/3.0.0).

Real-ESRGAN Anime uses x4plus_anime_6B; Anime Video uses the smaller
realesr-animevideov3 network. Both selected exports are FP32 4×. Animation training
does not establish fidelity for small manga lettering or screentones.
[Official project](https://github.com/xinntao/Real-ESRGAN),
[anime models](https://github.com/xinntao/Real-ESRGAN/blob/master/docs/anime_model.md).

Anime4K uses CNN S 2×, extracted from the official upscale shader and executed
through CUDA/ONNX. It is a fast display enhancement preset, without the complete
mpv restore/denoise/shader pipeline. Anime4K targets clean anime video rather than
low-resolution scan restoration. Real-ESRGAN and Anime4K exports come from
rigaya's pinned 20260808 release; their upstream weight licenses are retained.
[Anime4K](https://github.com/bloc97/Anime4K),
[export tools and models](https://github.com/rigaya/HWEnc-onnx-models).

AnimeSharp uses Kim2091's original 4× model, an interpolation of UltraSharp and
TextSharp, in FP32 opset 16. The model has CC-BY-NC-SA-4.0 terms. It emphasizes
sharp anime edges and can alter thin lines. It is the heaviest installed preset
in these timing checks. [Creator's model card and exports](https://huggingface.co/Kim2091/AnimeSharp).

## Validation and reproduction

All catalog downloads were installed through the Rust downloader with SHA-256
verification. CUDA checks ran three passes for all 17 exports on the original
600×900 procedural fixture. An additional CUDA test changed source heights within
one upscaler to check automatic session replacement, non-divisible edge tiles,
and alpha preservation. Unit checks cover corrupted weights, cancellation,
bounded ZIP expansion, unsafe manifests, and resolution-to-checksum selection.
Native Wayland checks covered the picker at 1240×900 and 820×600, plus
an actual IllustrationJaNai FP16-enhanced page displayed in the reader.

These tests establish integration and basic output behavior. They do not establish
restoration quality on a broad manga collection. Comparing lettering, fine lines,
screentones and tile boundaries across real pages remains useful before choosing
a preferred preset.

```sh
./scripts/run.sh --install-model all .local/models
python3 scripts/check-upscalers.py .local/models input.png /tmp/suwa-model-checks
```

Use a new output directory. Full model sources, hashes, byte ranges and tile
parameters are in [the catalog](../models/catalog.json); license texts are under
[models/licenses](../models/licenses). See [GPU setup](performance.md) for the
runtime requirements. The app uses Rust inference and does not invoke Python
while reading.
