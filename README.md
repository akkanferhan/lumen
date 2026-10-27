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

## Games and BIOS files

Lumen never downloads games or BIOS files. Use dumps of games and hardware you own.
Put them in the folders Lumen creates:

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
