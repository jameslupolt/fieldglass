# iNaturalist Screensaver — Requirements & Technical Plan (v3)

## Overview

A cross-platform (macOS + Windows) screensaver that displays research-grade nature photos from iNaturalist, filtered by the user's location and taxonomic interests. Photos cycle in a full-screen crossfade slideshow with species info always visible via a bottom bar overlay.

**Target audience**: Nature enthusiasts — comfortable with settings, but expects a polished, easy-to-install experience.

**Distribution**: Free, open-source via GitHub.

**Product goal**: Show aesthetically pleasing, biodiverse, nearby nature — not a random sample, not just "popular" photos.

---

## Architecture

### Three-Component Design

The product is **not** a single binary. It consists of three components sharing a common Rust core:

```
┌─────────────────────────────────────────────────────┐
│                    Shared Rust Core                  │
│   (iNaturalist API, cache, selection, metadata)       │
└──────────┬──────────────┬───────────────┬────────────┘
           │              │               │
    ┌──────▖──────┐ ┌─────▖──────┐ ┌──────▖──────┐
    │ macOS .saver│ │  Windows   │ │    Tauri    │
    │  (Swift +   │ │   .scr     │ │  Companion  │
    │  prototype- │ │ (Rust +    │ │    App      │
    │  dependent) │ │  WebView2) │ │ (settings,  │
    │             │ │  direct    │ │  preview,   │
    │ reads cache │ │  crate dep │ │  tray, cache│
    │ from disk   │ │            │ │  writes     │
    └─────────────┘ └────────────┘ └─────────────┘
```

| Component | Purpose | Technology | Consumes Core Via |
|-----------|---------|------------|-------------------|
| **Shared Rust Core** | API access, image cache, selection/diversity scoring, metadata, settings persistence | Rust library crate (`fieldglass-core`) | — |
| **macOS Screensaver Host** | Native `.saver` plugin — full OS screensaver lifecycle, preview, configuration hook | Swift + (WKWebView or Core Animation; decided by prototype) | Direct disk reads (cache directory + SQLite metadata) |
| **Windows Screensaver Host** | Native `.scr` — handles `/s`, `/c`, `/p HWND` protocol correctly | Rust binary + WebView2 | Direct crate dependency |
| **Tauri Companion App** | Settings UI, preview, cache management, tray icon, background refresh, auto-updates | Tauri v2 + React + TypeScript | Direct crate dependency |

### Why Not a Single Binary?

The previous plan proposed a single Tauri binary with thin OS "shims" for screensaver registration. That architecture fails because:

1. **macOS**: Apple's `.saver` bundles are `ScreenSaverView` plugins loaded by the system process. They are not thin launchers — the OS expects lifecycle methods (`startAnimation`, `stopAnimation`, `animateOneFrame`), preview rendering, and `configureSheet` for settings. A shim that spawns an external process cannot satisfy the preview contract or "Options..." button integration.

2. **Windows**: The OS invokes `.scr` files with `/s` (fullscreen), `/c` (config dialog), and `/p <HWND>` (render preview into a host-provided window handle). The preview mode requires rendering into a child window embedded in the Screen Saver Settings dialog — not launching a separate fullscreen window.

3. **Separation of concerns**: The screensaver hosts need minimal permissions and tiny footprints. The companion app needs network access, filesystem access, tray integration, and a rich settings UI. These are fundamentally different security profiles.

### Shared Rust Core (`fieldglass-core`)

All network access, data processing, and cache management lives here. Neither the screensaver hosts nor the React frontend make any network requests directly.

**Responsibilities:**
- iNaturalist API client (observations, taxa autocomplete)
- Image download and cache management (filesystem operations)
- Observation metadata storage and retrieval
- Selection and diversity scoring algorithm
- Settings persistence (read/write JSON config)
- Photo license validation
- Geocoding integration (Photon / Nominatim fallback)

**Interfaces:**
- Rust crate API (for Tauri companion and Windows `.scr`)
- Writes cache images + SQLite metadata to a shared app data directory that the macOS `.saver` reads directly from disk
- C FFI with `cbindgen` (deferred to post-v1; only needed if the macOS host requires live Rust calls beyond disk reads)

### macOS Screensaver Host

A real `ScreenSaverView` plugin distributed as a `.saver` bundle.

**⚠️ This is the gating prototype — not a normal feature build.** The macOS `.saver` + WKWebView combination is the highest-risk component. The workarounds below are based on community-observed behavior (webviewscreensaver, Aerial) rather than stable platform guarantees. Implementation must begin with a technology spike to validate feasibility before committing to a full build.

**Two implementation tracks (co-equal, decide after prototype):**

**Track A — WKWebView (preferred if it works):**
- Swift `ScreenSaverView` subclass
- Embeds `WKWebView` that loads HTML/CSS/JS bundled inside the `.saver`
- The web content is a minimal, self-contained slideshow renderer (no React — just vanilla HTML/CSS/JS)
- **CSS transitions only** — `requestAnimationFrame()` and `setInterval()` are broken/throttled in the `legacyScreenSaver` host process on Sonoma+. CSS animations are GPU-driven and work correctly.
- `configureSheet` returns an `NSWindow` sheet with an "Open Settings" button that calls `NSWorkspace.shared.open()` to launch/focus the companion app
- Handles preview mode (small WKWebView in the ScreenSaverView preview frame)
- Handles full-screen mode (borderless WKWebView covering the screen)

**Track B — Native Core Animation (fallback, likely more robust long-term):**
- Swift `ScreenSaverView` subclass
- Uses `NSImageView` or `CALayer` to display photos with Core Animation crossfade transitions
- Overlay text rendered via `NSTextField` or `CATextLayer`
- No WebView in full-screen mode at all — eliminates all WKWebView quirks
- Optionally uses WKWebView only for the small preview surface (lower risk at that size)
- Slightly more Swift code, but zero dependency on WebView timing/rendering reliability
- **This is not a distant contingency. The screensaver renderer is visually simple (image + crossfade + text overlay) — exactly what Core Animation is built for.**

**Data access (both tracks):** The `.saver` reads cached images and metadata **directly from disk** (cache directory + SQLite metadata). No live IPC, no FFI calls, no local HTTP server in the screensaver path. The companion app writes to the cache; the screensaver reads from it. This is the simplest trust boundary, easiest to debug, and requires no service lifecycle management inside the `.saver` host process.

**Critical Sonoma+ workarounds (MANDATORY, both tracks):**
- **`stopAnimation()` is NOT called** when the user dismisses the screensaver. Must listen for `com.apple.screensaver.willstop` distributed notification instead:
  ```swift
  DistributedNotificationCenter.default.addObserver(
    self, selector: #selector(willStop(_:)),
    name: Notification.Name("com.apple.screensaver.willstop"), object: nil)
  ```
- **Instance accumulation**: Each screensaver activation creates a new `ScreenSaverView` instance WITHOUT killing old ones. Old instances continue animating invisibly, leaking CPU/GPU. Must implement instance lame-ducking:
  ```swift
  // On init: post notification to kill older instances
  NotificationCenter.default.post(name: .newInstance, object: self)
  // Listen for newer instances and self-terminate
  NotificationCenter.default.addObserver(self, selector: #selector(neuter(_:)),
    name: .newInstance, object: nil)
  ```
- **Resource cleanup**: Release ALL resources (WKWebView/CALayer, timers) in the `willStop` handler, not in `stopAnimation()`

**Known challenges (Track A only):**
- WKWebView `requestAnimationFrame()` is broken in `legacyScreenSaver` process — use CSS transitions exclusively
- WKWebView `setInterval()` is throttled to ~1 call/sec — photo timer must use CSS `animation-delay` or a Swift-side `Timer` that messages JS via `evaluateJavaScript`
- Some users report black screen in full-screen mode while preview works (Issue #77 on webviewscreensaver) — no complete fix; CSS-only rendering may mitigate
- The `.saver` runs in the `legacyScreenSaver` host process — sandboxing restrictions apply
- Must be code-signed and notarized for distribution outside the App Store

### Windows Screensaver Host

A Rust binary compiled as a `.scr` (renamed `.exe`).

**Implementation:**
- Parses command-line flags: `/s`, `/c`, `/p <HWND>` (via `std::env::args()`)
- `/s` — Creates borderless fullscreen windows (one per monitor) using `windows-rs` crate, embeds WebView2 via `wry` crate, loads bundled slideshow HTML
- `/c` — Opens/activates the companion app's settings window (finds companion process, sends activation signal, or launches if not running)
- `/p <HWND>` — Parses HWND from argv, creates a child window within it via `SetParent`. **Does NOT use WebView2 for preview.** Renders a static thumbnail of the most recent cached photo + small attribution text using native GDI/Direct2D. This avoids WebView2 complexity in a ~300×200px embedded pane where a full slideshow adds failure modes for negligible user value.
- Calls Rust core directly as a crate dependency for cached images and metadata
- Handles multi-monitor by enumerating displays via `EnumDisplayMonitors` and creating one `WS_POPUP | WS_EX_TOPMOST` window per monitor
- **Input dismissal**: Handles `WM_MOUSEMOVE`, `WM_KEYDOWN`, `WM_LBUTTONDOWN` etc. Ignores the FIRST `WM_MOUSEMOVE` after activation to prevent immediate dismissal from pre-existing cursor position.

**Known challenges:**
- `.scr` files receive heightened scrutiny from Windows Defender/SmartScreen — EV code signing certificate recommended for immediate trust
- WebView2 runtime must be present on the system (Tauri docs indicate availability from Windows 10 v1803 onward; exact minimum version should be verified against our support floor)
- WebView2 may fail in Session 0 (logon screen context) — for user-session screensavers this is not an issue

### Multi-Monitor Support

| Platform | Mechanism |
|----------|-----------|
| **macOS** | The system instantiates one `ScreenSaverView` per display automatically. Each instance independently reads from the shared cache. |
| **Windows** | The `.scr` in `/s` mode enumerates `EnumDisplayMonitors`, creates one borderless window per display, each with its own WebView2 instance. Each pulls different photos from the shared cache. |
| **Companion (preview)** | Single window on the primary display. |

---

## Tech Stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Shared core | **Rust** (library crate) | Type-safe, fast, produces Rust crate for direct consumption |
| macOS host | **Swift** + **(WKWebView or Core Animation; decided by prototype)** | Native `.saver` requires Swift/ObjC. Renderer TBD: Track A = WKWebView, Track B = native CALayer/NSImageView |
| Windows host | **Rust** + **WebView2** (via `wry` crate) | Rust-native, direct crate dep on core. `wry` is Tauri's own WebView wrapper — battle-tested, avoids raw C++ WebView2 SDK |
| Companion app | **Tauri v2** | Multi-window, tray icon, store plugin, updater, autostart |
| Companion frontend | **React 19 + TypeScript** | Settings UI, companion preview |
| Screensaver renderer | **Vanilla HTML/CSS/JS** (Track A) or **Core Animation** (Track B) | Track A: bundled inside `.saver` and `.scr`. Track B (macOS only): no WebView, native rendering |
| Slideshow animations | **CSS transitions** | Hardware-accelerated crossfades, minimal CPU |
| Styling (companion) | **Tailwind CSS** | Settings UI only |
| API | **iNaturalist API v1** (no auth) | Public read-only |
| Geocoding | **Photon** (photon.komoot.io), runtime-configurable | Free, no API key, search-as-you-type. Nominatim as fallback. Geocoder backend must be switchable at runtime without a software update (per OSMF policy for Nominatim fallback). |
| FFI tooling | **cbindgen** (deferred to post-v1) | For future C FFI if macOS host needs live Rust calls. v1 uses direct disk reads. |
| Build workspace | **Cargo workspace** | Multi-crate: `fieldglass-core`, `fieldglass-companion` (Tauri), `fieldglass-scr-windows` |
| macOS build | **Xcode project** | Builds the `.saver` bundle |
| CI/CD | **GitHub Actions** | Automated cross-platform builds, release artifacts |

---

## iNaturalist API Integration

### No Authentication Required

All endpoints used are public read-only. No API key, OAuth, or JWT needed.

### Rate Limits

- ~1 request per second (60/min)
- ~10,000 requests per day
- Media download: <5 GB/hour, <24 GB/day
- Custom `User-Agent` header **required** (e.g., `iNatScreensaver/1.0 (contact@email.com)`)

### Primary Endpoint: Observation Search

```
GET https://api.inaturalist.org/v1/observations?
  lat={latitude}
  &lng={longitude}
  &radius={km}
  &taxon_id={id}
  &quality_grade=research
  &photos=true
  &photo_license=cc-by,cc-by-nc,cc-by-sa,cc-by-nd,cc-by-nc-nd,cc-by-nc-sa,cc0
  &without_term_value_id=19,25,26,27   # Exclude: Dead, Scat, Track, Bone (server-side)
  &per_page=200
  &page={random_page}
```

**Changes from v1 plan:**
- `per_page=200` (max allowed) instead of 50 — more efficient cache fills, more diversity per pull
- `photo_license` expanded to include `-nd` variants — we display unmodified images, so NoDerivatives licenses are permissible. If crop-to-fill is enabled, ND-licensed photos must be excluded at display time.
- `without_term_value_id=19,25,26,27` — server-side exclusion of Dead, Scat, Track, and Bone annotations. **Prototype-validated pending**: community reports suggest `without_term_value_id` behavior can be unintuitive in some combinations. This parameter is a best-effort optimization, not a correctness guarantee. Client-side filtering is the true correctness layer.
- Additional client-side filtering in Rust for edge cases not caught by server-side exclusion

### Content Filtering Strategy

**Problem**: Combining multiple `term_id` parameters in a single iNaturalist API query is unreliable. Community reports confirm this is a known limitation — the API does not cleanly support AND logic across different annotation types in one request.

**Solution**: High-confidence filters server-side + all annotation semantics client-side.

```
API query (server-side — high-confidence filters only):
  quality_grade=research          ← reliable
  photos=true                     ← reliable
  photo_license=...               ← reliable
  without_term_value_id=19,25,26,27  ← best-effort optimization (see caveat below)

Rust post-processing (client-side — correctness layer):
  For each observation in results:
    - Check annotations array for "Alive or Dead" (term_id=17)
      → Exclude if value is "Dead" (term_value_id=19)
    - Check annotations array for "Evidence of Organism" (term_id=22)
      → Exclude if value is not "Organism" (term_value_id=24) and filter is enabled
    - Check photo license against user preferences
    - Run diversity scoring algorithm
```

**Caveat on `without_term_value_id`**: We will test whether this parameter behaves consistently for our target queries during prototyping. Until verified, annotation exclusion should be treated as a best-effort bandwidth optimization rather than a correctness guarantee. The Rust-side filter is the authoritative correctness layer regardless.

**Why this split:**
- Server-side `quality_grade`, `photos`, and `photo_license` are documented, reliable, and reduce data transfer significantly
- Server-side `without_term_value_id` may reduce unwanted results, but its behavior is not fully established by authoritative docs — treat as optimization
- Client-side filtering is the correctness boundary: it inspects the annotation data in the API response and makes deterministic keep/exclude decisions
- Most observations lack annotations entirely (~75%+ unannotated) — these pass through both layers since there's no annotation to exclude. This maximizes the photo pool while filtering known-bad content.

### Annotation Reference

| Term ID | Term Name | Value ID | Value | Meaning |
|---------|----------|----------|-------|---------|
| 17 | Alive or Dead | 18 | Alive | Organism was alive |
| 17 | Alive or Dead | 19 | Dead | Organism was dead |
| 17 | Alive or Dead | 20 | Cannot Be Determined | Unknown |
| 22 | Evidence of Organism | 24 | Organism | Photo shows the organism itself |
| 22 | Evidence of Organism | 25 | Scat | Photo shows scat/droppings |
| 22 | Evidence of Organism | 26 | Track | Photo shows tracks/footprints |
| 22 | Evidence of Organism | 27 | Bone | Photo shows bones/remains |
| 22 | Evidence of Organism | 28 | Molt | Photo shows molted skin/feathers |
| 22 | Evidence of Organism | 29 | Gall | Photo shows galls |
| 22 | Evidence of Organism | 30 | Egg | Photo shows eggs |
| 22 | Evidence of Organism | 31 | Hair | Photo shows hair/fur |
| 22 | Evidence of Organism | 32 | Leafmine | Photo shows leaf mines |
| 22 | Evidence of Organism | 35 | Construction | Photo shows nests/webs/etc. |
| 1 | Life Stage | 2 | Adult | Adult organism |
| 1 | Life Stage | 6 | Larva | Larval stage |
| 1 | Life Stage | 7 | Egg | Egg stage |
| 1 | Life Stage | 8 | Juvenile | Juvenile organism |

### Taxa Autocomplete

```
GET https://api.inaturalist.org/v1/taxa/autocomplete?q={search_term}
```

Called by Rust core, results piped to companion UI via Tauri commands. The React frontend does **not** call this directly.

### Image Sizes

| Size | Dimensions | Use Case |
|------|-----------|----------|
| `square` | 75×75 | Thumbnails in settings UI |
| `small` | 240px longest side | Preview in companion |
| `medium` | 500px longest side | Fallback |
| `large` | 1024px longest side | Standard displays |
| `original` | ≤2048px longest side | High-DPI / 4K displays |

**Pattern**: `https://static.inaturalist.org/photos/{id}/{size}.{ext}`

### Selection & Diversity Algorithm

**Problem**: `order_by=votes` biases toward charismatic megafauna and already-popular observations. This works against the product promise of showing "local nature around you."

**Solution**: A diversity-aware selection algorithm in the Rust core.

```
Fetch pipeline:
  1. Query API with random page selection (for variety across fetches)
  2. Score each candidate observation:

     score = photo_quality_score
           + taxon_diversity_bonus      (prefer under-represented taxa in cache)
           + observer_diversity_bonus   (prefer variety of photographers)
           + recency_bonus              (prefer recent observations, slight weight)
           - duplicate_penalty          (strongly penalize same species already cached)
           - same_observer_penalty      (reduce weight if observer already well-represented)

  3. Sort by score, select top N to fill cache
  4. Shuffle final cache order for display randomness
```

**Randomization strategy** (since API has no `order_by=random`):
1. Initial request: get `total_results` count for the query
2. Calculate max reachable pages: `min(total_results / per_page, 50)` (API limits deep paging to ~10,000 results)
3. Pick random pages across the range
4. Rotate which pages are selected on each refresh cycle

**Explicit design stance**: The product optimizes for "aesthetically pleasing nearby biodiversity" — a blend of visual quality and taxonomic variety, not raw popularity or pure randomness.

### Attribution Requirements (Mandatory)

The legal object being reused is the **photo**, not the observation record. Attribution follows Creative Commons best practices.

Every displayed photo **must** show:
- **Photo creator name** (e.g., "© Jane Smith") — NOT "observer" — the person who took the photo
- **License code** (e.g., "CC BY-NC 4.0")
- **Source mark** (e.g., "via iNaturalist")

**Metadata stored per cached photo** (for attribution, companion details panel, and legal compliance):
- Photo creator / display name
- License code + license URL
- Observation URL (link to iNaturalist page)
- Photo source URL
- Common name + scientific name
- Place name / location description
- Observation date
- Taxon ID (for diversity scoring)
- Observer username (for diversity scoring)
- Photo dimensions (for display quality decisions)
- Full attribution string (pre-formatted for overlay display)

**License handling:**
- Default: include CC BY, CC BY-NC, CC BY-SA, CC BY-NC-SA, CC BY-ND, CC BY-NC-ND, CC0
- CC BY-ND and CC BY-NC-ND are permissible because we display photos **unmodified** (letterbox/contain mode)
- If user enables crop-to-fill (which adapts the image), ND-licensed photos are filtered out at display time
- The overlay always displays the license code — this satisfies the "indicate the license" CC requirement

---

## Location Input

### Geolocation Strategy

**Problem**: The browser Geolocation API requires a secure context. Tauri v2 on Windows uses `http://tauri.localhost` by default, which is not a secure context. Enabling `useHttpsScheme` has compatibility implications. Browser geolocation is not a reliable cross-platform approach for Tauri.

**Solution**: Layered approach — do NOT depend on browser geolocation.

| Method | Priority | Implementation | Privacy |
|--------|----------|----------------|---------|
| **Manual city search** | Primary (v1) | Text input with autocomplete via Photon geocoding → returns lat/lng | City name sent to Photon (komoot.io); no precise location shared |
| **Manual coordinates** | Primary (v1) | Direct lat/lng entry for users who know their coords | No external service needed |
| **Tauri geolocation plugin** | Secondary (v1, prototype-gated) | `@tauri-apps/plugin-geolocation` — uses native OS location services (CoreLocation / Windows.Devices.Geolocation). **Caveat**: official docs show mobile-scoped setup examples; desktop behavior needs prototype verification before treating as turnkey. Requires explicit permission handling. | Precise location stays on device; only lat/lng sent to iNaturalist |
| **Interactive map** | v2 | Leaflet.js + OpenStreetMap tiles — click-to-set location | Tile requests go to tile provider |

### Geocoding Architecture

The geocoder is implemented as a **`Geocoder` trait** in `fieldglass-core` with pluggable backends. The active backend must be **switchable at runtime** (via settings config) without requiring a software update. This is an explicit OSMF operational requirement for any app that uses Nominatim as a fallback.

#### Primary: Photon (by Komoot)

- **Free**: No API key required
- **Autocomplete-friendly**: Supports search-as-you-type (unlike Nominatim, which explicitly prohibits autocomplete)
- **No explicit rate limits**: Fair-use policy ("be reasonable")
- **Privacy**: Uses OSM data; no API key means less user tracking
- **OSS-compatible**: Apache 2.0 license, self-hostable on Elasticsearch
- **Risk**: Shared public service with no SLA — Komoot may change availability

#### Fallback: Nominatim (OpenStreetMap)

- **Single-shot geocoding only** — autocomplete is explicitly forbidden by OSMF policy
- **Rate limit**: 1 request/second (hard limit)
- **Operational requirements** (per OSMF Nominatim Usage Policy):
  - Must send a valid identifying `User-Agent` header (app name + contact)
  - Must cache repeated results locally
  - Must be able to switch geocoding service without a software update (hence the trait/adapter pattern)
  - Must not bulk-geocode or send periodic/automated requests
- Used only when Photon is unavailable or user explicitly selects it in settings

#### Why not Nominatim as primary?
Nominatim's usage policy explicitly states: "No autocomplete allowed." Since our primary UX is a debounced search-as-you-type field, Nominatim cannot serve as the primary geocoder. Photon provides the same OSM data with autocomplete support.

### v1 Location UI

- Text field with debounced autocomplete (Photon search, 300-500ms debounce)
- Results dropdown showing city/region/country
- Selected location displayed as text + coordinates
- Radius slider: presets at 10 / 25 / 50 / 100 km (default: 50 km)
- Optional "Use my location" button (via Tauri geolocation plugin, if OS permissions granted). **Prototype-gated**: verify desktop behavior before shipping.
- No map in v1 — text-based location selection is sufficient and avoids tile provider dependencies

---

## Features

### v1 — MVP

#### Screensaver Display

- **Full-screen crossfade slideshow** — hardware-accelerated CSS transitions
- **Photo duration** — configurable (default: 15s); user-settable in companion
- **Aspect ratio** — default: **contain/letterbox** (black bars, preserves full image and ND-license compatibility). User option: crop-to-fill (note: ND-licensed photos are excluded when crop-to-fill is active, since cropping constitutes adaptation under CC BY-ND / BY-NC-ND). **No background blur** — applying a blurred version of the photo behind letterboxed images would constitute an adaptation and is incompatible with ND-licensed photos. If blur is added in v2, ND-licensed photos must be excluded from that rendering mode.
- **Bottom bar overlay** (semi-transparent, always visible):
  - Common name + scientific name (italicized)
  - Photo creator attribution ("© Creator Name")
  - License badge (e.g., "CC BY-NC 4.0")
  - Place name (from observation)
  - "via iNaturalist" source mark
- **Multi-monitor** — one screensaver instance per display, independent photo streams from shared cache
- **Exit on input** — any mouse movement (ignoring first move after activation) or keyboard press dismisses the screensaver

#### Location Input (see Location Input section above)

- Primary: manual city search via Photon + direct coordinates
- Secondary: Tauri geolocation plugin for precise location
- Radius: 10 / 25 / 50 / 100 km presets (default: 50 km)

#### Taxa Picker

- **Autocomplete search** — powered by `/taxa/autocomplete`, called from Rust core
- **Quick presets** — common groups: All Life, Plants, Animals, Fungi, Birds, Insects, Reptiles & Amphibians, Fish, Mammals
- **Multiple taxa** — up to ~5 active selections in standard mode; "advanced" mode for more
- **Taxa info display** — show icon and observation count for selected taxa

#### Content Filtering

All filters toggleable in companion settings:

| Filter | Where Applied | Default | Purpose |
|--------|--------------|---------|---------|
| Research grade only | API query | ON | Community-verified observations |
| Licensed photos only | API query | ON | Only CC-licensed photos |
| Exclude dead organisms | Rust post-processing | ON | Filters annotation data when present |
| Exclude non-organism evidence | Rust post-processing | ON | Filters scat, tracks, bones when annotated |

#### Image Cache

**Target-based sizing** — cache size is computed, not hard-coded:

```
required_images = (target_no_repeat_minutes × 60 × monitor_count) / photo_duration_seconds
```

| Scenario | Target | Monitors | Duration | Required Cache |
|----------|--------|----------|----------|----------------|
| Default | 30 min | 2 | 15s | 240 images |
| Single monitor | 30 min | 1 | 15s | 120 images |
| 3 monitors | 30 min | 3 | 15s | 360 images |

- **Default target**: 30 minutes without repeats
- **Configurable**: User sets "no-repeat time" slider in companion (15–60 minutes)
- **Refresh cycle**: Daily — companion app checks for new photos once per 24 hours
- **Cache rotation**: Score-based — replace lowest-diversity-score images first
- **Offline support**: Screensaver works from cache when offline
- **Storage location**: `{app_data}/inaturalist-screensaver/cache/`
- **Metadata store**: **SQLite** database alongside cached images. The data model already supports scoring, representation counts, filtering, history, replacement policy, license checks, and future view/favorite/blacklist state — this is database-shaped from the start. JSON is simpler only for toy-sized data; our model is past that threshold.
- **First-run behavior**: Screensaver is not registered/enabled until first successful cache fill completes

#### Cache Concurrency Semantics

The companion app writes to the cache while one or more screensaver instances may be reading from it simultaneously (multiple monitors, plus the companion itself for preview). This requires explicit concurrency rules:

**Image file writes (companion → disk):**
- **Atomic writes via rename**: Companion downloads images to a temp file (`{cache}/tmp/{uuid}.tmp`), then atomically renames to final path (`{cache}/images/{id}.jpg`). This ensures screensaver instances never read a partially-written file.
- **No in-place overwrites**: Images are never modified after being written. Replacement = write new file + delete old file.

**SQLite access (companion writes, screensaver reads):**
- **WAL mode** (Write-Ahead Logging): Enables concurrent readers and a single writer without blocking. The companion is the only writer; screensaver instances are read-only.
- **Read-only connections** for screensaver hosts: Open SQLite with `SQLITE_OPEN_READONLY` flag. This prevents accidental writes and avoids lock contention.
- **Connection lifetime**: Screensaver opens a read-only connection on start, closes on stop. Does not hold transactions open across photo transitions.

**Photo deletion eligibility:**
- A cached photo is eligible for deletion only during a cache refresh cycle (companion-initiated).
- Before deleting, the companion marks the photo as `pending_deletion` in SQLite.
- On next refresh, photos marked `pending_deletion` are removed from disk if they have been marked for longer than the current screensaver session duration (conservative: 1 hour). This avoids deleting a photo that a screensaver instance is currently displaying.
- Alternative (simpler): never delete during an active screensaver session. Only delete when the screensaver is not running. Detect via process enumeration or a heartbeat file.

**Multi-monitor photo selection (avoiding duplicates across displays):**
- Each screensaver instance maintains a local "display queue" — a shuffled copy of the available photo IDs from SQLite, read at startup.
- Instances are **not coordinated** with each other. On a 2-monitor setup with 240 cached photos, the probability of simultaneous display is `1/240` per transition — acceptably low.
- If exact deduplication is required (v2): use a shared memory-mapped file or a coordination row in SQLite where each instance claims photo IDs. But this adds complexity and is not needed for v1.

#### Companion App (System Tray / Menu Bar)

- **Tray icon** — always accessible
- **Quick menu**:
  - Preview screensaver
  - Refresh cache now
  - Open settings
  - Cache status (e.g., "235/240 photos cached, last refreshed 3h ago")
  - Quit
- **Settings window** — full configuration UI (see below)
- **Background operation** — manages daily cache refresh, stays running quietly
- **Auto-start** — prompts user after first successful sync; toggle in settings (not silently forced)

#### Settings UI (in Companion App Window)

All settings are in the Tauri companion app. The React frontend is a **dumb renderer** — it receives typed view models from Rust via Tauri commands and sends user actions back. It makes **no** network requests or filesystem operations directly.

- **Location section**: City search (autocomplete), coordinates display, radius presets, "Use my location" button
- **Taxa section**: Autocomplete search, preset buttons, selected taxa chips (max ~5 standard, "advanced" for more)
- **Display section**: Photo duration slider, aspect ratio (contain vs fill), overlay opacity
- **Content section**: Toggle filters (research grade, licensed, alive only, organism only)
- **Cache section**: No-repeat time slider, storage usage display, clear cache button, manual refresh
- **About section**: Version, photo creator credits, iNaturalist attribution, license info, links

**Tauri capability scoping**: Screensaver windows get fewer permissions than the companion window — no network access, no filesystem write. Companion windows get full permissions for settings, cache management, and API access.

#### Platform Integration

**macOS:**
- `.saver` bundle installed to `~/Library/Screen Savers/` — appears in System Settings > Screen Saver
- "Options..." button in System Settings triggers `configureSheet` → opens/focuses companion app
- Preview in System Settings shows miniature slideshow via WKWebView in the preview frame
- `.saver` must be code-signed and notarized

**Windows:**
- `.scr` file installed to system directory — appears in Screen Saver Settings
- `/c` flag opens/focuses companion app's settings window
- `/p <HWND>` renders miniature slideshow preview in the host-provided window
- `.scr` should be code-signed to avoid SmartScreen warnings

### v2 — Roadmap Features

| Feature | Description | Notes |
|---------|------------|-------|
| **Favorites** | Keyboard shortcut during screensaver saves photo to favorites | Store observation IDs locally; weight cache selection toward favorites |
| **Blacklist** | Keyboard shortcut hides a photo permanently | Store observation IDs to exclude; skip during cache refresh |
| **Seasonal awareness** | Prioritize observations from the current month/season | Add `month={current_month}` to API query; toggle in settings |
| **Auto-updates** | Check for and install updates automatically | Tauri updater plugin + GitHub Releases |
| **Interactive map** | Click-to-set location on a map in settings | Leaflet.js + OpenStreetMap tiles (requires tile provider attribution) |
| **Transition variety** | Additional effects (slide, zoom, fade-to-black) | User-selectable in settings; CSS-based |
| **Hotkey info expand** | Key press during screensaver shows expanded details | Modal overlay with full observation data, link to iNaturalist page |
| **Photo quality preference** | Prefer higher-resolution photos | Filter by photo dimensions metadata |
| **Details panel** | Companion app panel showing details of current/selected cached photo | Observation URL, full attribution, map of observation location |

---

## Project Structure

```
inaturalist-screensaver/
├── Cargo.toml                           # Workspace root
├── crates/
│   ├── fieldglass-core/                 # Shared Rust core library
│   │   ├── Cargo.toml                   # [lib] crate-type = ["lib"] (staticlib deferred to post-v1)
│   │   ├── src/
│   │   │   ├── lib.rs                   # Public API
│   │   │   ├── ffi.rs                   # C FFI exports (deferred to post-v1; only if macOS needs live Rust calls)
│   │   │   ├── api/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── client.rs            # HTTP client, rate limiting, User-Agent
│   │   │   │   ├── observations.rs      # Observation search + response parsing
│   │   │   │   ├── taxa.rs              # Taxa autocomplete
│   │   │   │   └── geocoding.rs         # Geocoder trait + Photon impl (Nominatim fallback)
│   │   │   ├── cache/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── manager.rs           # Cache lifecycle (fill, rotate, cleanup)
│   │   │   │   ├── storage.rs           # Filesystem operations
│   │   │   │   └── metadata.rs          # Observation metadata persistence
│   │   │   ├── selection/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── diversity.rs         # Diversity scoring algorithm
│   │   │   │   └── filter.rs            # Client-side annotation filtering
│   │   │   ├── config/
│   │   │   │   ├── mod.rs
│   │   │   │   └── settings.rs          # Settings persistence (read/write JSON)
│   │   │   └── types.rs                 # Shared types (Observation, Photo, Taxa, etc.)
│   │   ├── cbindgen.toml                # C header generation config (deferred to post-v1)
│   │   └── build.rs                     # Build script (future: C header generation)
│   │
│   ├── fieldglass-companion/            # Tauri companion app
│   │   ├── Cargo.toml                   # Depends on fieldglass-core
│   │   ├── tauri.conf.json
│   │   ├── capabilities/               # Tauri v2 capability files
│   │   │   ├── companion-window.json    # Full permissions
│   │   │   └── preview-window.json      # Minimal permissions
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── lib.rs                   # Tauri setup, commands, plugins
│   │   │   ├── commands/                # Tauri IPC commands
│   │   │   │   ├── mod.rs
│   │   │   │   ├── settings.rs          # Get/set settings
│   │   │   │   ├── cache.rs             # Cache status, manual refresh
│   │   │   │   ├── taxa.rs              # Taxa search (delegates to core)
│   │   │   │   ├── location.rs          # Geocoding (delegates to core)
│   │   │   │   └── photos.rs            # Get cached photos for preview
│   │   │   └── tray.rs                  # System tray setup and menu
│   │   └── icons/
│   │
│   └── fieldglass-scr-windows/         # Windows .scr screensaver host
│       ├── Cargo.toml                   # Depends on fieldglass-core
│       ├── src/
│       │   ├── main.rs                  # Entry point: parse /s /c /p flags
│       │   ├── fullscreen.rs            # /s mode: multi-monitor fullscreen
│       │   ├── preview.rs               # /p mode: render into host HWND
│       │   ├── config.rs                # /c mode: launch companion app
│       │   └── webview.rs               # WebView2 setup and management
│       └── resources/
│           └── slideshow/               # Bundled HTML/CSS/JS for slideshow
│               ├── index.html
│               ├── slideshow.js
│               └── slideshow.css
│
├── macos/                               # macOS .saver Xcode project
│   ├── iNatScreenSaver.xcodeproj/
│   ├── iNatScreenSaver/
│   │   ├── iNatScreenSaverView.swift    # ScreenSaverView subclass
│   │   ├── Info.plist
│   │   ├── iNatScreenSaver-Bridging-Header.h  # Swift-Rust bridge (only if FFI path is chosen post-prototype)
│   │   └── Resources/
│   │       └── slideshow/               # Bundled HTML/CSS/JS (Track A) or empty (Track B)
│   │           ├── index.html
│   │           ├── slideshow.js
│   │           └── slideshow.css
│   └── build-rust.sh                    # Script to build Rust .a for macOS targets (deferred to post-v1 unless FFI path chosen)
│
├── frontend/                            # React frontend (companion app only)
│   ├── package.json
│   ├── tsconfig.json
│   ├── tailwind.config.js
│   ├── src/
│   │   ├── App.tsx                      # Companion app shell
│   │   ├── main.tsx                     # React entry point
│   │   ├── pages/
│   │   │   ├── Settings.tsx             # Main settings page
│   │   │   ├── Preview.tsx              # Screensaver preview
│   │   │   └── About.tsx                # Credits and info
│   │   ├── components/
│   │   │   ├── LocationSettings.tsx     # City search, radius, coordinates
│   │   │   ├── TaxaPicker.tsx           # Taxa autocomplete + presets
│   │   │   ├── DisplaySettings.tsx      # Duration, aspect ratio, overlay
│   │   │   ├── ContentFilters.tsx       # Quality, licensing, annotation toggles
│   │   │   └── CacheStatus.tsx          # Cache size, usage, refresh
│   │   ├── types/
│   │   │   └── models.ts               # View model types (mirror of Rust types)
│   │   └── lib/
│   │       └── commands.ts              # Typed Tauri invoke wrappers
│   └── index.html
│
├── slideshow/                           # Shared slideshow renderer (vanilla)
│   ├── index.html                       # Slideshow HTML template
│   ├── slideshow.js                     # Crossfade logic, photo cycling
│   ├── slideshow.css                    # Transitions, overlay styling
│   └── README.md                        # How the slideshow protocol works
│
├── .github/
│   └── workflows/
│       ├── ci.yml                       # Lint, test, build check
│       └── release.yml                  # Build + sign + release artifacts
│
├── PLAN.md                              # This file
└── README.md
```

### Key Structural Decisions

1. **`slideshow/`** is shared between macOS `.saver` (Track A only) and Windows `.scr`. If macOS uses Track B (native Core Animation), the slideshow HTML is only bundled into the Windows `.scr`.

2. **`frontend/`** is the React app used **only** by the Tauri companion. It is a dumb renderer — all logic flows through Tauri commands to the Rust backend.

3. **`crates/fieldglass-core/`** produces a Rust library crate. C FFI (`staticlib` + `cbindgen`) is deferred to post-v1 — the macOS `.saver` reads cached images and SQLite metadata directly from disk in v1, requiring no live Rust calls.

4. **The React frontend has NO:**
   - API client (`api.ts`) — REMOVED
   - Direct fetch hooks (`useObservations.ts`, `useTaxa.ts`) — REMOVED
   - Network access of any kind
   - Filesystem access

   It communicates exclusively via Tauri `invoke()` commands that return typed view models.

---

## Build & Distribution

### Workspace Build

```bash
# Build all Rust crates
cargo build --workspace

# Build only the core library
cargo build -p fieldglass-core

# Build Windows .scr
cargo build -p fieldglass-scr-windows --release

# Build Tauri companion (includes frontend)
cd crates/fieldglass-companion && npm run tauri build

# Build macOS .saver (requires Xcode)
cd macos && ./build-rust.sh && xcodebuild -project iNatScreenSaver.xcodeproj -scheme iNatScreenSaver
```

### Development

```bash
# Companion app development
cd crates/fieldglass-companion && npm run tauri dev

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace
cd frontend && npm run lint
```

### Artifacts Per Platform

**macOS release**:
- `iNatScreensaver.dmg` containing:
  - `iNat Companion.app` (Tauri companion)
  - `iNatScreenSaver.saver` (screensaver plugin)
  - Install instructions

**Windows release**:
- `iNatScreensaver-setup.msi` containing:
  - `iNat Companion.exe` (Tauri companion, installed to Program Files)
  - `iNatScreenSaver.scr` (installed to System32 or user-local)
  - Registry entries for screensaver registration

### Code Signing & Notarization (First Release Requirement)

Code signing is **not optional** for user-facing distribution. Unsigned apps trigger scary OS warnings that will stop non-technical users cold.

| Platform | Requirement | Cost | Notes |
|----------|------------|------|-------|
| **macOS** | Developer ID Application certificate + Notarization via `notarytool` | $99/year (Apple Developer Program) | Both `.app` and `.saver` must be signed and notarized |
| **Windows** | Code signing certificate (EV or standard) | $200–400/year | Builds SmartScreen reputation. EV cert provides immediate trust. |

**Plan**: Budget for signing from the start. Integrate signing into the GitHub Actions release workflow via secrets. Do not ship unsigned builds to non-developer users.

### GitHub Actions CI/CD

- **On push to main**: `cargo test`, `cargo clippy`, `npm run lint`, build check
- **On tag (vX.Y.Z)**: Full release pipeline:
  - Build macOS `.dmg` (signed + notarized)
  - Build Windows `.msi` (signed)
  - Create GitHub Release with assets
  - Generate changelog

### Installation

**macOS:**
1. Download `.dmg` from GitHub Releases
2. Open `.dmg`, drag `iNat Companion.app` to Applications
3. Double-click `iNatScreenSaver.saver` → macOS prompts to install
4. Launch `iNat Companion` → appears in menu bar
5. Configure location + taxa in settings
6. Companion fills cache → screensaver becomes active

**Windows:**
1. Download and run `.msi` installer
2. Companion app installs to Program Files, `.scr` installs to system directory
3. Companion appears in system tray
4. Configure location + taxa in settings
5. Companion fills cache → screensaver appears in Screen Saver Settings

### Update Flow (Multi-Artifact)

Updates must keep all components compatible. The Tauri updater plugin handles companion app updates (via GitHub Releases + static JSON metadata), but **replacing the `.saver` and `.scr` files are custom installer steps** that the Tauri updater does not handle automatically.

Strategy:

1. Companion app checks for updates on launch (Tauri updater plugin → GitHub Releases)
2. If update available, companion downloads the full update package (all components bundled)
3. Tauri updater handles replacing the companion app binary
4. **Custom post-update step (macOS)**: Companion runs a script or prompts the user to reinstall the updated `.saver` bundle. Exact `.saver` install semantics are OS-mediated (double-click prompts System Settings confirmation) and must be verified during prototyping. The user may need to confirm.
5. **Custom post-update step (Windows)**: Companion copies the updated `.scr` to the system directory. May require elevation (UAC prompt). Alternatively, install `.scr` to the user-local `%APPDATA%\Microsoft\Windows\Screen Savers\` directory to avoid elevation.
6. Core library is statically linked into all binaries — no shared library versioning issues
7. **Semantic versioning enforced in CI** — all components are versioned together to guarantee compatibility

---

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| macOS `ScreenSaverView` + WKWebView instability on Sonoma+ | High | **CRITICAL**: `stopAnimation()` is NOT called on dismiss (must use `willstop` notification). Instance accumulation leaks CPU/GPU (must implement lame-ducking). `requestAnimationFrame()` and `setInterval()` are broken (must use CSS transitions exclusively). Monitor `webviewscreensaver` project for fixes. Fallback: native `CALayer`/`NSImageView` + Core Animation crossfades if WebView is unworkable. |
| Windows `.scr` preview mode (`/p`) | Low | Preview renders a static thumbnail with attribution text (no WebView2). Full slideshow only in `/s` mode. |
| iNaturalist rate limiting | Low | Cache aggressively (240+ images), refresh once/day, respect 1 req/sec, set proper User-Agent |
| Unsigned app triggers OS warnings | High | **Budget for code signing from the start.** Apple Developer ($99/yr), Windows signing cert ($200-400/yr). Integrate into CI/CD. |
| Inappropriate nature photos slip through | Medium | Rust-side annotation filtering + research-grade requirement. User blacklist in v2. Accept that filtering is best-effort when annotations are sparse. |
| Photon geocoding downtime | Low | Cache geocoding results locally. Fallback to Nominatim for single-shot geocoding (no autocomplete). Ultimate fallback: direct lat/lng entry. |
| No internet on first run | Low | Explicit setup flow: companion shows onboarding screen, does not enable screensaver until first cache fill succeeds |
| 4K displays need high-res photos | Low | Use `original` size (≤2048px) for high-DPI; `large` (1024px) for standard. Photo may still be lower resolution than display — contain mode with black bars is the mitigation. No background blur (incompatible with ND-licensed photos). |
| Multi-monitor edge cases | Medium | macOS handles this natively (one ScreenSaverView per display). Windows requires explicit monitor enumeration — test with 2–3 monitor setups, fallback to primary-only if enumeration fails. |
| Most observations lack annotations | Medium | Annotation filters are best-effort: if no annotation present, observation is included (not excluded). This maximizes the photo pool while still filtering known-bad content. |
| Multi-artifact update compatibility | Medium | All components are statically linked against the same core library version. Updates always bundle all components together. Semantic versioning enforced in CI. |
| macOS `.saver` data access constraints | Medium | v1 uses direct disk reads from cache (image files + SQLite metadata). No live IPC, no FFI calls, no local HTTP in the screensaver path. If disk reads are blocked by `legacyScreenSaver` sandboxing, fall back to companion-served local HTTP. |
| Photon geocoding is a shared service | Low | No SLA or guaranteed availability. Self-hosting is possible (Apache 2.0, Elasticsearch backend) if Photon goes down. Cache results aggressively — users only geocode once during setup. |
| Windows .scr immediate dismissal on existing cursor | Low | Ignore first `WM_MOUSEMOVE` after screensaver activation. Standard pattern used by all Windows screensavers. |

---

## Resolved Questions

These were open in v1 of the plan. Decisions made:

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| 1 | Photo duration default | **15 seconds**, user-configurable | Standard for photo screensavers. Cache size adjusts to compensate. |
| 2 | Search radius default | **50 km** with presets (10/25/50/100 km) | Good general default. Presets let urban users narrow and rural users widen. |
| 3 | Maximum taxa selections | **~5 in standard mode**, unlimited in advanced | Prevents UI clutter while allowing power users full control. |
| 4 | Companion auto-start | **Prompt after first successful sync**, toggle in settings | Respects user agency. Not silently forced. |
| 5 | Photo aspect ratio | **Contain/letterbox by default**, crop-to-fill as option | Preserves species integrity. Keeps ND-license compatibility. User can opt into fill. |
| 6 | iNaturalist branding | **Wordmark in text** ("via iNaturalist"). Logo only in clearly referential contexts. | Per iNaturalist help: logo OK when linking, not for implying endorsement. No sale of logo items. |
| 7 | Offline-first first run | **Explicit setup flow**. Screensaver not enabled until first cache fill completes. | Prevents blank/broken screensaver experience. Clear onboarding. |

## Remaining Open Questions

1. **macOS WKWebView vs native Core Animation**: The gating prototype. Build a minimal `.saver` with both Track A (WKWebView) and Track B (native CALayer) to determine which is viable on Sonoma+. This is week-1 work.

2. **`without_term_value_id` behavior verification**: Test the server-side annotation exclusion parameter with representative queries to confirm it behaves as expected. Until verified, client-side filtering is the correctness layer.

3. **macOS `.saver` reinstallation during updates**: Test whether the Tauri updater can seamlessly reinstall the `.saver` bundle, or whether user interaction is required. This affects the update UX.

4. **Shared slideshow renderer maintenance**: The vanilla JS slideshow (if Track A is chosen) is used by both macOS `.saver` and Windows `.scr`. Ensure changes are tested in both contexts. Consider a build step that copies from `slideshow/` into both host resource directories.

---

## Implementation Priority Order

Stage implementation to validate the highest-risk components first:

| Phase | What | Why First |
|-------|------|-----------|
| **1. macOS prototype** | `.saver` lifecycle, dismissal cleanup, preview, image display, attribution overlay, no runaway instances. Test both Track A (WKWebView) and Track B (native). | Highest technical risk. Gating decision for the entire macOS path. |
| **2. Windows prototype** | `/s` fullscreen, `/c` config launch, `/p` static preview, input dismissal, multi-monitor enumeration. | Second-highest risk. Must validate WebView2 in `.scr` context + preview HWND embedding. |
| **3. Rust core + cache + metadata** | API client, cache manager, SQLite metadata, diversity scoring, Photon geocoder. Finalize disk layout and schema only after host constraints from phases 1–2 are clear. | Core business logic. Shaped by what the hosts actually need. |
| **4. Companion app** | Tauri companion with settings UI, tray, preview, cache management. | Lowest risk, most familiar tech. Should not drive architecture. |

**Do NOT build the companion app first.** It is the easiest part and should adapt to host constraints, not the other way around.
