# First run on a 12 GB NVIDIA card

The `rtx3060` preset is for a Windows PC with a 12 GB NVIDIA card, 64 GB of RAM, and a Direct3D 12 device that supports shader model 6.6 and mesh shaders. It does not ship a game. You point it at a dump you already own.

Build the runner from this tree. The launcher will not start a prebuilt executable sitting next to it.

```powershell
cargo build --release --locked -p victory
```

## Settings the cache must match

Shader translations and the driver's pipeline cache depend on the `VICTORY_*` switches and on the card (wave width, view-heap size). Translate and warm with the same switches you will play with. `Play-VIctory.cmd rtx3060` sets them. In the same window, before the first play:

```powershell
.\Play-VIctory.cmd rtx3060
```

stops at the game if `VICTORY_GAME_DIR` is unset, so set the variables yourself when you only want the cache filled:

```powershell
$env:VICTORY_TEXTURE_LOW = '2'
$env:VICTORY_CARD_FETCH_INFLIGHT = '0'
$env:VICTORY_FIRSTLANE_OWN = '0'
$env:VICTORY_RAW_VIEWS = '0'
$env:VICTORY_MEMORY_CARD_MB = '2048'
$env:VICTORY_TEXTURE_STAGE_MB = '192'
$env:VICTORY_POOL_SPARES = '8'
$env:VICTORY_KEEP_FREED = '0'
$env:VICTORY_RIPE_BLOCKS = '4'
$env:VICTORY_CACHE_DIR = "$env:LOCALAPPDATA\VIctory\cache-pareto"
```

Leave `VICTORY_NO_COMPILE` unset. That hash is for an AMD driver that never finishes one compile.

## Checklist

1. **Translate shaders once.** `target\release\victory.exe shaders <your shader archives>` opens the card and probes the view heap before it writes the cache, so the key matches a later run with these switches. This is the long step. It can be stopped and continued.
2. **Play.** `.\Play-VIctory.cmd rtx3060` after `VICTORY_GAME_DIR` points at your dump. The same environment is `scripts\play.ps1 -Rtx3060 -Build release -Exe <your executable>`.
3. **Warm pipelines after a run has drawn.** `target\release\victory.exe warm --used` makes the pipelines that run used, into the driver's on-disk cache, so the next launch spends less time compiling them. The first launch still stutters while that cache is empty. A panicked compile finishes its queue entry instead of holding the worker until the title's watchdog fires.
4. **Do not record a frame in the world.** The title's own watchdog ends the process when a capture stalls a frame. `warm` and `shaders` do not record frames.
5. **Dark rooms.** Indirect light that the title traces is off unless you ask for it. `VICTORY_FIRSTLANE_OWN` stays `0`: turning it on has corrupted ray bounds. Expect dim interiors until that path is safe.
6. **Memory.** The preset keeps less spare card memory and gives empty blocks back instead of holding them for five minutes. The title itself still wants a large commit on the Series X profile; this preset selects Series S. If other programs need the RAM, close them before launching. Unset `VICTORY_KEEP_FREED` or set it to anything but `0` to restore the longer retention.

`VICTORY_VIEW_LIMIT` is optional. Leave it unset so the card's own view-heap size is used. Set it to `999999` only if a larger heap is accepted and then hangs on an index past a million views.
