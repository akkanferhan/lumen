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
Master System, Mega Drive, Saturn, Dreamcast, Arcade (FBNeo), PlayStation 1/2/3, PSP, Xbox, Xbox 360.

## Games and BIOS files

Lumen never downloads games or BIOS files. Use dumps of games and hardware you own.
Put them in the folders Lumen creates:

```
<game folder>/
├─ snes/  ps2/  gc/ ...   ← games, one folder per system
└─ bios/
   ├─ ps1/  ps2/  ps3/  xbox/  dc/
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

## Branching

- `main`: releases only
- `develop`: integration branch
- `feature/*`: one branch per piece of work, merged into `develop`
