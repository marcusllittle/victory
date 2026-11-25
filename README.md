# victory — rorodev6's build

A Windows runtime for **an Xbox Series X|S title**, run from game files you own yourself. This branch is maintained by
**[rorodev6](https://github.com/rorodev6)**: it combines several community forks into one build tuned and tested on an
**AMD Radeon RX 6700 XT (12 GB)**, and adds low-memory texture loading so the game fits a 12 GB card.

The title's original x86-64 code runs unchanged inside the process; the runtime provides the Xbox/GDK platform beneath it
and translates the console's GPU data and shaders (D3D12.X / GFX10 → D3D12 / DXIL).

> No game data is included or distributed here. You need your own copy of the game.

## What this build is

| Part | Source |
|------|--------|
| Base runtime | [Marvisak/VIctory](https://github.com/Marvisak/VIctory) |
| World rendering, lighting, relay and many shader fixes | [uiliannn/VItoriaBr](https://github.com/uiliannn/VItoriaBr) (commit `2b3c339`) |
| Draw windows, copy merging, late waits, driver watch | [fxceless/victory-fork](https://github.com/fxceless/victory-fork) (hyphennn) |
| Low textures, merge of the above, AMD tuning, Discord status | **rorodev6** (this branch) |

### rorodev6's changes

- **`VICTORY_TEXTURE_LOW` (1 or 2):** leaves out the largest mip levels of the title's textures so ~15 GB of textures fit a
  12 GB card. Without it, textures waiting for room show up purple or black and assets fail to appear.
- **Low textures and tiled textures work together:** a low texture is made whole at its reduced size instead of tiled. Tiles
  count the host texture's levels while views, uploads and copies shift the title's levels by the left-out mips; doing
  both shifted a view twice.
- **Merged the VItoriaBr and fxceless lines** (copy statistics from both sides; descriptor views shift by the tiles' first
  level, then by the left-out mips).
- **Discord rich presence, an "alive" check and the driver watch** carried over from the fxceless line.
- A tested AMD preset (below).

## Status (2026-11-25, RX 6700 XT 12 GB, Ryzen, 32 GB RAM)

| Stage | State |
|-------|-------|
| Boot, menus, intro video | ✅ Works (videos up to ~40 fps) |
| Profile and saves | ✅ Works |
| 3D world: opening cinematic, prologue, open world | 🟡 Runs at **~3.2 fps** |
| Characters | 🔴 Some heads, hair and beards missing (POPS pixel shaders not translated yet) |
| Textures | 🟡 Mostly fine with `TEXTURE_LOW=2`; a few purple ones while an area loads |
| Shaders | 25,985 containers, ~240 still do not translate |

The frame time is mostly the emulation itself (GPU work at 2560x1440 internally, copying memory back to the title, and the
submitting thread waiting on a lock), not a single expensive setting.

## Build

```powershell
copy victory.example.toml victory.toml        # set paths outside the repository
cargo build --profile diag -p victory --features diagnostics
.\target\diag\victory.exe warm <game_dir>     # optional: pre-translate every shader into the cache (~10 min)
```

## Recommended preset (RX 6700 XT 12 GB)

```powershell
$env:VICTORY_CACHE_DIR        = "<a folder outside the repo>"
$env:VICTORY_DATA_DIR         = "<a folder outside the repo>"   # saves
$env:VICTORY_TEXTURE_LOW      = "2"       # textures fit 12 GB
$env:VICTORY_INDIRECT_WINDOW  = "1"       # if the game loses the GPU, remove this first
$env:VICTORY_LATE_WAIT        = "1"       # stable level on this card
$env:VICTORY_CARD_REHASH_RUNS = "1"
$env:VICTORY_KEEP_VIEWS       = "0"
$env:VICTORY_WIDE_BACK        = "1"
$env:VICTORY_WIDE_REFRESH     = "1"
$env:VICTORY_WIDE_MERGE       = "1"
$env:VICTORY_JOIN_COPIES      = "1"
$env:VICTORY_TEXTURE_NO_WAIT  = "1"       # if a texture flickers, remove this first
$env:VICTORY_NO_COMPILE       = "39f0e9aeb156a61c"   # a shader the AMD driver never finishes compiling
# $env:VICTORY_DISPLAY_MODE   = "1920x1080"          # optional; the title still renders at 2560x1440 inside

.\target\diag\victory.exe run "<game_dir>\<title_exe>" --execute
```

## Credits

Built on the work of Marvisak (VIctory), uiliannn and the VItoriaBr contributors, and hyphennn (fxceless) and the
victory-fork contributors. The previous README of the base is kept as [README.upstream.md](README.upstream.md).

## License

GPL-3.0-or-later, like the projects it is based on. See [LICENSE](LICENSE).
