# Feature checklist

This is a pre-release implementation, not a declaration of full Tsumiru parity.
The reference is [Suwayomi-Tsumiru](https://github.com/Suwayomi/Suwayomi-Tsumiru).

| Area | Implemented | Still needed / limits |
| --- | --- | --- |
| Native interface | Rust, Iced, Wayland, wgpu; desktop entry and icon | Clean-machine packaging and broader compositor testing |
| Connection | Address validation; Basic, UI, Simple auth; token renewal; optional headers; read-only mode; Secret Service sign-in storage and automatic reconnect | Named profile UI, network discovery, reverse-proxy/cookie fixtures beyond current tests |
| Library | Paginated fetching; search; category, unread, downloaded, bookmark and status filters; hidden categories and their titles; title, recent-read, latest-chapter, count and added-date sorting | Bulk manga actions, more layout options |
| Sources | Popular/latest/search, pagination, native checkbox/text/select/sort/tristate/group filters; source preferences | Global search, source migration, external-extension coverage; filter groups are queried to depth four |
| Extensions | Repository add/remove; extension install, upgrade, uninstall | APK sideloading, detailed permissions/status UI |
| Manga | Metadata, library membership, category assignment, chapter refresh, continue reading | Per-series reader overrides, richer metadata/link handling |
| Chapters | Search, unread/bookmark filtering, direction toggle, paged lists, individual and bulk local download/read actions | Arbitrary range selection, bulk bookmark/server download actions |
| Reader | RTL/LTR/vertical paged modes, continuous webtoon, gaps, fit modes, zoom, spread, fullscreen, prefetch, progress and retry | Margin crop, rotation, brightness/contrast, configurable shortcuts/tap zones, touchpad pinch, spread offset |
| GPU | 11 described presets across waifu2x, MangaJaNai/IllustrationJaNai, Real-ESRGAN, Anime4K and AnimeSharp; verified optional downloads; automatic MangaJaNai resolution choice; CUDA FP32/FP16 inference; tiled work; cancellation; original-first rendering | Vulkan/AMD/Intel neural backends, automatic runtime installer, broad manga quality comparison suite |
| Offline | Scoped metadata/images, opportunistic page cache, retained earlier saved chapters | Local download controls and automatic local downloading intentionally removed; new downloads use Suwayomi's server queue |
| Progress | Persistent outbox, revision-safe acknowledgments, conflict choices, explicit read/unread, incognito, tracker retry queue | Broader multi-device stress testing; server API updates lack compare-and-swap |
| Categories | Create, rename, default, assign, reorder, delete with confirmation; local hide/unhide per server account | Per-category sort settings |
| Updates/history | Native lists, open chapter, trigger/stop library update | Pagination beyond the first 100 entries, live subscriptions |
| Tracking | Accounts, login/logout, OAuth browser handoff and pasted callback, search/bind/edit/unbind; chapter progress propagation | Real provider account validation, automatic callback handling; provider credentials are session-only |
| Backups | Export flags, streamed download, file validation, restore confirmation/progress; 2.3 and 2.4 schema compatibility | Native file picker, scheduled backup management |
| Appearance | Pastel goth light/dark, built-in theme presets, custom accent, OLED background, cache/prefetch settings | Accessibility audit, localization, complete keyboard focus review |

## Validation boundaries

The isolated integration suite passes against Suwayomi 2.3.2243 and 2.4.2366 for
the core library/download/progress/category/backup flows. Source-option query
compatibility is tested with the built-in local source. External extensions and
tracker providers need separate validation with their actual services.

The application's GPU path has run on the target RTX 4080 Super. Performance
figures are described in [performance.md](performance.md). There is no controlled
Tsumiru comparison yet, and no claim that Suwa is faster or lighter in every case.
