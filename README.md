# Suwa

A native Linux manga reader for Suwayomi, written in Rust with Iced and wgpu.
The interface renders directly on Wayland. There is no browser engine or embedded
Suwayomi WebUI. Suwayomi still handles sources, extensions, and server downloads.

This is a working pre-release. Full Tsumiru parity is unfinished; see
[the feature checklist](docs/features.md) for what works and what remains.

## Run

```sh
git clone https://github.com/may-gay/suwa.git
cd suwa
cargo build --locked --release --features cuda
./scripts/run.sh
```

The launcher uses the local CUDA runtime under `.local/gpu` when present. Runtime
files are excluded from Git; set up ONNX Runtime, CUDA and cuDNN using the
[GPU setup instructions](docs/performance.md). The launcher checks for source
changes and rebuilds when needed.

To add Suwa to your application runner from this checkout:

```sh
python3 scripts/install-desktop.py
```

This installs `~/.local/share/applications/app.suwa.Suwa.desktop` (or uses
`XDG_DATA_HOME` when set). It launches `scripts/run.sh` from this checkout, so
source changes are built before Suwa opens and each launch uses the latest build.
Run the installer again if you move the checkout.

For the native reader without neural enhancement:

```sh
cargo run --locked --release
```

A working Wayland session and Vulkan driver are required. The development machine
uses Arch Linux, Hyprland, and an NVIDIA RTX 4080 Super. Rust's build tools and a C
compiler are needed to build; SQLite is compiled into the executable. The native
AVIF decoder needs `dav1d` and `pkgconf` installed when building.

Enter the Suwayomi address in **Connection**. Supported login methods are HTTP
Basic, Suwayomi UI login, and Simple login. **Remember sign-in on this device**
saves the password and optional HTTP headers in your Linux keyring after a
successful connection, then reconnects automatically on launch. It uses
`secret-tool` from `libsecret` and a Secret Service provider such as GNOME Keyring.
If the keyring is unavailable, the connection still works and Suwa reports that
sign-in was not saved. Uncheck the option to remove this account's saved sign-in;
the current session remains connected. **Use saved sign-in** restores a previously
saved account after entering its server, login method, and username. Changing
those fields clears the previous account's password and headers from the form.
Secrets are scoped by server, login method, username, and local Suwa profile;
they are not written to the settings database. Connection settings and reading
data are stored locally. Read-only is enabled initially. Disable it and reconnect for source
browsing, fetching uncached chapters, and syncing reading progress.

Chapter downloads use Suwayomi's server queue. Click the down arrow beside a
chapter, or use **Edit** to select chapters and **Download on server**. The sidebar's
**Downloads** page controls the server queue. Suwa caches pages while reading;
previously saved local chapters are retained, but local-download controls and
background local downloading have been removed.

Chapter lists deduplicate releases by default. Under **Edit**, use **Deduplicate
chapters** to show or hide duplicates and **Preferred release group** to choose a
scanlator. Automatic selection prefers the group covering the most distinct
chapters and fills gaps from other groups. Read and in-progress copies take
priority so you keep your place. The chapter list, continue reading, and reader
navigation all use this selection. Preferences are saved per series and server
account. For example, Anything! Okay's 72 entries become 40 chapters, using Gamma
for all 40 with automatic selection.

In **Settings → Categories**, enable **Hide category and its manga** to hide that
category tab and its manga from normal library browsing. A title in multiple
categories is hidden if any one of them is hidden. Searching your library still
shows matching hidden titles. **Filter → Show hidden categories** temporarily
reveals hidden category tabs and their manga; turn it off, reset filters, or
restart Suwa to hide them again. Hidden category preferences are saved locally
per server account and work offline.

Each category also has **Show in All titles**. Uncheck it to leave that category's
series out of the **All titles** tab, including searches within that tab. Its own
category tab stays available. A series in any unchecked category is excluded,
even if it also belongs to a checked category. The default category controls
uncategorized series. All categories are included initially; new categories are
included automatically. These selections are saved locally per server account,
work offline, and remain in effect when resetting filters or showing hidden
categories. Source browsing is unaffected.

Thumbnails support JPEG, PNG, WebP, GIF, and AVIF. **Retry covers** retries failed
thumbnails; invalid cached images are discarded so a retry can fetch a fresh copy.

## GPU enhancement

**Settings → Upscaling** has a model picker with descriptions, intended material,
license, and download size. Use **Install and use** for an optional model or
**Use this model** for installed weights. The choice persists across restarts.
Downloads run off the UI thread, support cancellation, verify SHA-256, and publish
weights atomically. Original pages appear first while CUDA enhancement runs in
the background. The reader enhances visible pages first, then up to the next
three pages by default as they load. Settings → Reading adjusts the lookahead;
it does not process a whole chapter at once.

There are 11 presets: bundled waifu2x Fast, CUnet, SwinUNet Art and Scan,
MangaJaNai Auto, IllustrationJaNai Denoise and Detail, Real-ESRGAN Anime 6B and
Anime Video, Anime4K CNN S, and AnimeSharp. MangaJaNai selects among seven 2×
source-height models for black-and-white pages. IllustrationJaNai uses the
compact V3 SPAN S 2× models. Anime4K runs an exported CNN through CUDA.
See [the model catalog and measured timings](docs/upscaling-research.md).

The launcher stores optional models under `.local/models`. When running the
binary directly, optional weights use the local Suwa data directory's `models`
folder. `SUWA_MODEL_DIR` overrides that location. Optional weights are excluded
from Git; install them through Settings or the model installer command.
The bundled fast model works offline. Optional models need a connection only to
install; reading and inference use the installed files. Custom manifests remain
available under **Custom model…**.

Build with `--features cuda` and provide a CUDA-enabled ONNX Runtime 1.24 or newer,
matching CUDA and cuDNN libraries. An unavailable runtime reports an error and
keeps the original page visible. See [GPU setup and performance](docs/performance.md).

## Appearance

The default design follows the [Pastel Goth reference](https://may-gay.github.io/styleguides/styles/pastel-goth.html):
plum surfaces, lavender and blush accents, pill buttons, rounded fields, and soft
hover highlights. Maple Mono is used throughout, with bundled regular and bold
fonts for offline rendering. Settings has theme presets, a custom accent, and an OLED background.
Library filters open from **Filter**. Series metadata and category assignment are
under **Details**, while chapter search and bulk actions are under **Edit**.
Bookmark and read-status actions live in each chapter's menu. Reader controls
show page navigation first, with fit, zoom, direction, and enhancement under **Options**.
The previous default Suwa palette migrates to Pastel goth on launch; custom accents
and other selected theme presets are preserved.

## Controls

- Scroll through the full library. Mouse Back and Forward buttons and browser
  Back and Forward keys move through visited screens.
- Arrow keys, Page Up / Page Down, and Space navigate pages or scroll webtoons.
- In webtoon mode, each mouse-wheel step moves about 180 pixels with a short
  glide; touchpad scrolling gets a smaller boost and keeps its fine control.
- Middle-click in webtoon mode to autoscroll. Move farther up or down to change
  speed; middle-click again, click another button, or press Escape to stop.
- F11 toggles fullscreen. H hides reader controls. Escape leaves the reader.
- When controls are hidden, hover at the top or bottom edge to slide that bar
  back in. Reader options include a compact model picker and install control.
- The reader offers right-to-left, left-to-right, vertical pages, webtoon, and
  webtoon with gaps; fit, zoom, and two-page controls are available in the toolbar.
- Webtoon reading continues into the next chapter and records chapter-local
  progress. Incognito disables new reading progress records.
- Settings shows reading conflicts when another device changed a pending position.
  Choose the local or server position explicitly.

## Development checks

```sh
cargo fmt --check
cargo test --locked --all-features
cargo clippy --locked --all-features --all-targets -- -D warnings
```

Loopback tests are ignored by default for environments without socket permission:

```sh
cargo test --all-features api::network_tests -- --ignored --skip isolated_suwayomi_roundtrip
SUWA_TEST_SERVER_JAR=/path/to/Suwayomi-Server-v2.3.2243.jar scripts/test-server.sh
```

The keyring check uses temporary test entries and removes them afterward:

```sh
cargo test --all-features credentials::tests::keyring_roundtrip -- --ignored
```

The integration script requires Java 21 and Python 3. It launches an isolated
server on an ephemeral loopback port, creates original fixture pages, and tests
library/chapter operations, local downloads, progress, categories, source options,
and backup export/validation/restore. It never uses your configured server. The
server process is stopped on exit; the printed temporary directory contains logs.
Set `SUWA_TEST_EXPORT` to a new directory to retain an offline UI fixture profile.

On Hyprland, `python3 scripts/smoke-wayland.py` checks a disposable native window
and captures only its rendered content. With an exported fixture profile, pass
`--profile PATH --view library`, `series`, `reader`, `webtoon`, or `settings`.
Add `--enhance` to the reader check to wait for an actual enhanced page.
Use `--size 1240x900` to float and resize only the test window.

## Files and packaging

Data lives under `$XDG_DATA_HOME/suwa` (normally `~/.local/share/suwa`). Override it
with `SUWA_DATA_DIR` to use a separate profile. SQLite stores settings, cached
metadata, pending progress, and the local download queue. Image files are scoped by
server, authentication method, and username. Downloaded pages are original files;
enhanced pages are held in memory. Source preference credentials are sent to
Suwayomi and are not saved by Suwa.

`packaging/PKGBUILD` builds from this local checkout. From `packaging`, use
`makepkg` to create an Arch package. The desktop file and puppy PNG icon are included.
The package does not bundle CUDA or ONNX Runtime. Packaging has not yet received a
clean-machine install test.

Source is licensed under [MPL 2.0](LICENSE). Bundled model weights are MIT-licensed;
see [models/LICENSE.waifu2x](models/LICENSE.waifu2x). Runtime libraries and Cargo
dependencies retain their own licenses. The bundled fonts use the SIL Open Font
License; see [assets/fonts](assets/fonts/README.md).
