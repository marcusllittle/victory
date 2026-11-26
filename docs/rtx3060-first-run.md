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
$env:VICTORY_AMBIENT_SETTLE = '250,3'
$env:VICTORY_CACHE_DIR = "$env:LOCALAPPDATA\VIctory\cache-pareto"
```

Leave `VICTORY_NO_COMPILE` unset. That hash is for an AMD driver that never finishes one compile.

## Checklist

0. **Check the PC.** `scripts\preflight.ps1` in the window you will play from, with the two paths set. It reads only: the build against the checked-out commit, the dump, the shader cache, the card and driver, Windows' GPU timeout, the commit limit and overlays that skew frame times. Fix its FAIL lines; read its WARN lines.
1. **Translate shaders once.** `target\release\victory.exe shaders` (with no paths it reads `VICTORY_GAME_DIR`) opens the card and probes the view heap before it writes the cache, so the key matches a later run with these switches. This is the long step. It can be stopped and continued.
2. **Play.** `.\Play-VIctory.cmd rtx3060` after `VICTORY_GAME_DIR` points at your dump. The same environment is `scripts\play.ps1 -Rtx3060 -Build release -Exe <your executable>`. `rtx3060-1080p` (or `-Output1080` with `-Rtx3060`) asks for a 1920x1080 output and a render scale of 1. That is not the default. Each patched place is compared to the bytes it was taken from and left unchanged when they differ. `VICTORY_FFX_SCALE` is not a switch here.
3. **Warm pipelines after a run has drawn.** `target\release\victory.exe warm --used` makes the pipelines that run used, into the driver's on-disk cache, so the next launch spends less time compiling them. The first launch still stutters while that cache is empty. A panicked compile finishes its queue entry instead of holding the worker until the title's watchdog fires.
4. **Do not record a frame in the world.** The title's own watchdog ends the process when a capture stalls a frame. `warm` and `shaders` do not record frames.
5. **Dark rooms.** Indirect light that the title traces is off unless you ask for it. The preset's stand-in (`VICTORY_AMBIENT=0.8`) follows the clock (`VICTORY_AMBIENT_SETTLE=250,3`) so a room change is not held for several seconds at a low frame rate. Unset `VICTORY_AMBIENT_SETTLE` to keep 0.04 of the way each frame. `VICTORY_FIRSTLANE_OWN` stays `0`: turning it on has corrupted ray bounds. Interiors the title would light by rays stay an estimate.
6. **Memory.** The preset keeps less spare card memory and gives empty blocks back instead of holding them for five minutes. The title itself still wants a large commit on the Series X profile; this preset selects Series S. If other programs need the RAM, close them before launching. Unset `VICTORY_KEEP_FREED` or set it to anything but `0` to restore the longer retention.

`VICTORY_VIEW_LIMIT` is optional. Leave it unset so the card's own view-heap size is used. Set it to `999999` only if a larger heap is accepted and then hangs on an index past a million views.

## Measuring a run

A release build shows no panel and the launcher logs warnings only, so ask for the numbers in a file. Every one of these is off unless set, and none changes what is drawn.

```powershell
$env:VICTORY_GAME_DIR  = 'D:\pixel\victory\game'
$env:VICTORY_CACHE_DIR = 'D:\pixel\victory\cache'
$env:VICTORY_STATS     = 'D:\pixel\victory\stats\run.csv'   # the folder must exist
$env:VICTORY_D3D12_DRED = 'faults'                           # a device removal names the faulting address
.\Play-VIctory.cmd rtx3060 2> D:\pixel\victory\stats\run.log
scripts\stats-summary.ps1 D:\pixel\victory\stats\run.csv -MinDraws 1000
```

`VICTORY_STATS` writes one row a second: frame rate, the mean and the longest frame of that second, draws, draws dropped for want of a pipeline, indirect records run empty, the submitting thread's time and its waits for the card, private bytes, working set, commit left, the card's memory used and budget and the PC memory it spilled into, and the texture queue. It never writes over a file (`run.csv` becomes `run-1.csv`) and puts the run's `VICTORY_*` switches beside it in `run.csv.switches.txt`. `scripts\play.ps1 -Stats <file>` sets it too. `-MinDraws 1000` keeps the seconds in the world, `-From <seconds>` leaves out the start.

Compare runs on the same save and the same part of it. Differences under about 5 % are noise.

## The four faults, and what to look at first

1. **Device removal.** The run ends with "host GPU device removed" in `run.log`. With `VICTORY_D3D12_DRED=faults` (release builds take it now; it costs next to nothing, and objects are named while it is on) a fault on the card shows `page fault at 0x...` with the allocations around it and the title memory copy or heap below it. `DXGI_ERROR_DEVICE_HUNG` with no page fault is a timeout. The message then names `TdrDelay`: at the default 2 s, set the DWORD `TdrDelay` to 10 under `HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers` and restart. "slow host submission" warnings over 1000 ms before it point the same way. Keep `run.log`, `run.csv` and `crashes.log` from the title's data folder.
2. **Foliage flicker.** Note the seconds it flickers and read the two columns for them. `dropped_no_pipeline` above 0 means draws were left out while their pipelines were made: run `victory warm --used` after that run and try again. `indirect_empty` above 0 means indirect records ran out of their list's budget. `VICTORY_INDIRECT_REST` is already off. Try `VICTORY_INDIRECT_BATCH=1` and then `VICTORY_INDIRECT_WINDOW=0`, one at a time. If both columns stay at 0 while it flickers, the cause is elsewhere: say so with the scene.
3. **Dark rooms without rays.** The stand-in's estimate follows what is on screen, so a room lit only by bounced light stays near black. `VICTORY_AMBIENT_FLOOR=0.1` (off unless set; 0.05 to 0.2) gives the estimate a least level, on the scale of `constant:<level>`, shaded in creases. It does not let more of the title's own leftover through. Raise it until the room reads, then check that night outdoors is not milky.
4. **Late textures.** `textures_waiting` that stays high, or `texture_queue_full_total` that keeps growing, means textures wait for room on the card, not for the disk. On 12 GB the queue keeps 3 GiB free for render targets (`VICTORY_CARD_ROOM_FOR_TARGETS_MB`, measured on the Series X profile at 1440p) and stops at 90 % of the budget (`VICTORY_TEXTURE_ROOM_PERCENT`). Series S makes smaller targets. Try `VICTORY_CARD_ROOM_FOR_TARGETS_MB=1536`, then `VICTORY_TEXTURE_ROOM_PERCENT=94`, and watch `shared_used_mb`: if it climbs past a few hundred MB the card is over budget and frames slow down. Queue full never, and textures still late, points at staging: `VICTORY_TEXTURE_STAGE_MB=256`.
