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

### Console mode and power

- **Console mode** (Menu): Lumen starts with Windows, fullscreen.
- **Power mode** (Menu, ROG Ally): switches the ASUS power mode per system through the same
  firmware interface Armoury Crate uses: Silent for 2D systems, Turbo for PS2/PS3/PS4/Xbox 360/
  Wii U/GameCube/Wii, Performance otherwise. Performance is restored when the game ends.

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

## Branching

- `main`: releases only
- `develop`: integration branch
- `feature/*`: one branch per piece of work, merged into `develop`
