# Field Glass

Turn your screen saver into a window to local wildlife.

Field Glass is a Windows screen saver that displays research-grade nature photos from [iNaturalist](https://www.inaturalist.org/), filtered by your location and taxonomic interests. Photos cycle in a full-screen crossfade slideshow with species identification and Creative Commons attribution always visible.

## Features

- **Location-aware** — set your coordinates and search radius to see wildlife observed near you
- **Taxonomic filtering** — focus on birds, plants, fungi, insects, or any group that interests you
- **Research-grade only** — shows only community-verified observations by default
- **Diversity-weighted selection** — prioritizes biodiversity over common species, so you see more than just robins and squirrels
- **Creative Commons attribution** — displays photographer credit and license for every photo
- **Multi-monitor support** — each display shows different photos simultaneously
- **Offline caching** — photos are downloaded in the background and displayed from a local cache
- **Companion app** — system tray app for configuring settings, previewing photos, and managing the cache

## Installation

Download the latest `.msi` installer from [GitHub Releases](https://github.com/jameslupolt/fieldglass/releases). The installer places:

- `FieldGlass.scr` in `C:\Windows\System32` (where Windows looks for screen savers)
- `Field Glass.exe` (companion app) in `C:\Program Files\Field Glass\`
- Start Menu shortcuts for the companion app and Screen Saver Settings

After installation, the companion app opens automatically. Set your location and any taxonomic filters, then select "Field Glass" in Windows Screen Saver Settings.

## How It Works

Field Glass queries the [iNaturalist API](https://api.inaturalist.org/v1/) (public, no authentication required) for research-grade observations with Creative Commons–licensed photos near your location. A diversity-weighted selection algorithm ensures you see a broad range of species rather than just the most commonly observed ones. Photos are cached locally so the screen saver works offline.

## Architecture

The project is a Cargo workspace with three crates sharing a common core:

| Crate | Purpose | Technology |
|-------|---------|------------|
| `fieldglass-core` | iNaturalist API client, image cache, selection algorithm, settings | Rust library |
| `fieldglass-scr-windows` | Native `.scr` screen saver — handles `/s`, `/c`, `/p` protocol | Rust + Win32 GDI |
| `fieldglass-companion` | Settings UI, photo preview, cache management, system tray | Tauri v2 + React |

## Building from Source

**Prerequisites**: Rust toolchain, Node.js 20+, .NET SDK (for WiX v4)

```sh
# Screen saver
cargo build -p fieldglass-scr-windows --release
copy target\release\FieldGlass.exe target\release\FieldGlass.scr

# Companion app
cd crates/fieldglass-companion
npm ci --prefix ../../frontend
cargo tauri build

# MSI installer (requires WiX v4)
wix build installer\windows\Product.wxs -arch x64 -ext WixToolset.UI.wixext ^
  -d ProductVersion=0.2.1.0 ^
  -d ScrPath=dist\windows\FieldGlass.scr ^
  -d CompanionPath=target\release\fieldglass-companion.exe ^
  -d IconPath=crates\fieldglass-companion\icons\icon.ico ^
  -d LicensePath=installer\windows\License.rtf ^
  -o dist\windows\FieldGlass.msi
```

## License

[MIT](LICENSE) — Copyright (c) 2026 James Lupolt

Nature photos displayed by the screen saver are sourced from iNaturalist and are licensed under their respective Creative Commons licenses. Attribution is displayed for every photo.
