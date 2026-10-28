# Lumen

A gamepad-first, fullscreen emulation frontend for Windows handhelds such as the ASUS ROG Ally.
One interface for every system: Lumen downloads and configures the emulators itself, scans your
game folders and launches games fullscreen — no separate emulator windows.

> Status: early development (pre-MVP).

## Features (planned for MVP)

- First-run setup in under a minute: pick a game folder, Lumen creates the system folders
- Automatic download of emulators and libretro cores from their official sources
- Emulator configs written for you: fullscreen, no first-run wizards, BIOS paths
- Box art from thumbnails.libretro.com
- Hold **Select + Start** in any game to return home
- Favorites, recently played and play time

## Supported systems

NES, SNES, N64, Game Boy / Color / Advance, Nintendo DS, 3DS, GameCube, Wii, Wii U,
Master System, Mega Drive, Saturn, Dreamcast, Arcade (FBNeo), PlayStation 1/2/3/4, PSP, Xbox, Xbox 360.

Installed **Steam**, **Epic Games** and **Xbox app** (Game Pass / Microsoft Store) titles appear in
the same library, read from the launchers' own files and started through them, so DRM, overlays and
cloud saves keep working. Xbox app games are found through each drive's `.GamingRoot` folder and
`MicrosoftGame.config`, with posters from the Microsoft Store catalog. Older UWP-only titles without
a `MicrosoftGame.config` aren't listed.

PS3 and PS4 games are folder dumps (`PS3_GAME` / `sce_sys`); their titles are read from `PARAM.SFO`.
PS4 support uses [shadPS4](https://github.com/shadps4-emu/shadPS4) and is experimental. There is no usable PS5 emulator,
so PS5 is not supported.

## Tuned for the ROG Ally

Lumen detects the chip (Z1, Z1 Extreme, Z2 A, Z2 Extreme) and writes tuned settings into every
emulator before each launch: Vulkan renderers, internal resolution per GPU tier, asynchronous shader
compilation, borderless fullscreen, controller mapping and no first-run wizards or update prompts.
Emulator settings never need to be opened. The tier can be overridden in Menu → Graphics profile.

Emulators keep their own state in their config files, so Lumen only sets the keys it owns
(read–merge–write for INI, TOML, YAML and JSON); Xenia gets its settings as command-line flags.

### Themes

Menu → Theme. Each theme brings its own library layout, type and navigation, not just colors:

| Theme | Layout |
|---|---|
| Işık | Cover grid (default) |
| Galeri | Full-screen game art, serif headlines, a cover rail along the bottom |
| Sade | Light theme: text list with a large preview, fastest for big libraries |
| Kartuş | Cartridge cards, pixel fonts and CRT scanlines |

### Console mode and power

- **Console mode** (Menu): Lumen starts with Windows, fullscreen.
- **Power mode** (Menu, ROG Ally): switches the ASUS power mode per system through the same
  firmware interface Armoury Crate uses: Silent for 2D systems, Turbo for PS2/PS3/PS4/Xbox 360/
  Wii U/GameCube/Wii, Performance otherwise. Performance is restored when the game ends.

## RetroAchievements

Sign in once (Menu → RetroAchievements) and achievements work in RetroArch, DuckStation, PCSX2,
PPSSPP and Dolphin. The password is sent to RetroAchievements once and never stored; only the
session token is kept (in `secrets.json`, excluded from backups) and written to each emulator in its
own format, including DuckStation's encrypted token. Hardcore mode is a single switch.

## Save backups

Each emulator's save and save-state folders (and Lumen's play data) are mirrored after every
session to `OneDrive\Lumen Yedek` when OneDrive is set up, otherwise `Documents\Lumen Yedek`;
the folder can be changed in Menu → Save backup. Copies are incremental. Restore copies back only
files that are newer in the backup. xemu (saves inside its HDD image) and melonDS (saves beside the
ROM) aren't covered yet.

## Homebrew store

Menu → Homebrew store lists free Game Boy, Game Boy Color, Game Boy Advance and NES homebrew from
[Homebrew Hub](https://hh.gbdev.io). Only games whose license explicitly allows redistribution
(MIT, GPL, BSD, Zlib, CC0, CC-BY family…) are included; entries without a license are left out. Each
game shows its license and source. Downloads go straight into the right system folder.

The catalog lives in `src-tauri/homebrew.json`; refresh it with `node scripts/homebrew-catalog.mjs`.

## Games and BIOS files

Lumen never downloads games or copyrighted BIOS files. Use dumps of games and hardware you own.
What Lumen does provide on its own:

| System | Without your BIOS |
|---|---|
| PS1 | Runs on PCSX ReARMed's open-source HLE BIOS (DuckStation is used once a BIOS is added) |
| Saturn | Runs on Yabause's HLE BIOS (Beetle Saturn is used once a BIOS is added) |
| PS3 | System software is downloaded from Sony's official update server (free) |
| Xbox | xemu's blank HDD image is downloaded; the MCPX and flash BIOS are still required |
| GBA, DS, Dreamcast, 3DS | Built-in BIOS replacements, nothing needed |
| PS2 | A BIOS dump is required (no open-source replacement runs games) |

Put your own files in the folders Lumen creates:

```
<game folder>/
├─ snes/  ps2/  gc/ ...   ← games, one folder per system
└─ bios/
   ├─ ps1/  ps2/  ps3/  ps4/  xbox/  dc/
   └─ (RetroArch BIOS files go in bios/ directly)
```

## Stack

- [Tauri 2](https://tauri.app): Rust native layer (downloads, process launching, XInput) + web UI
- React + TypeScript + Vite for the interface

## Development

Requirements: Node 20+, [Rust](https://rustup.rs).

```sh
npm install
npm run tauri dev
```

Emulator installs and launching only work on Windows; on macOS/Linux the UI runs for development.

## License

Lumen is licensed under the [GNU General Public License v3.0](LICENSE).

## Installing on a handheld

Every push to `develop` builds Windows installers. Open the latest
[Windows build](../../actions/workflows/release.yml) run, download the `lumen-windows-…` artifact and run the
`.exe` installer. The builds are not code-signed yet, so Windows SmartScreen asks for confirmation
("More info" → "Run anyway").

## Releasing

1. Bump `version` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Merge `develop` into `main` and push a tag `vX.Y.Z`.
3. The workflow creates a **draft** release with the installers and the signed `latest.json`.
   Publishing the draft rolls the update out: Lumen checks for it on start and offers it in the menu.

Auto-update needs the `TAURI_SIGNING_PRIVATE_KEY` repository secret (the private key of the
public key in `tauri.conf.json`). Without it, builds still work but ship no update files.

## Branching

- `main`: releases only
- `develop`: integration branch
- `feature/*`: one branch per piece of work, merged into `develop`
