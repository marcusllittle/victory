@echo off
setlocal EnableExtensions DisableDelayedExpansion
rem pareto-residency: Series S and the residency settings measured on RTX.
rem rtx3060: that preset, plus a 12 GB NVIDIA card (64 GB system RAM is enough;
rem the limit is the card). Builds from this tree only: target\release\victory.exe,
rem or target\diag\victory.exe when the release binary is not there.
if not "%~1"=="" if /i not "%~1"=="pareto-residency" if /i not "%~1"=="rtx3060" (
  echo Usage: Play-VIctory.cmd pareto-residency ^| rtx3060 1>&2
  exit /b 2
)
if not defined VICTORY_GAME_DIR (
  echo Set VICTORY_GAME_DIR to your own dump folder. 1>&2
  exit /b 2
)
if not exist "%VICTORY_GAME_DIR%\game_scarlett_master.exe" (
  echo VICTORY_GAME_DIR has no game_scarlett_master.exe. 1>&2
  exit /b 2
)
set "runner=%~dp0target\release\victory.exe"
if not exist "%runner%" set "runner=%~dp0target\diag\victory.exe"
if not exist "%runner%" (
  echo Build it first: cargo build --release --locked -p victory 1>&2
  exit /b 2
)
if not defined VICTORY_AMBIENT_AIR set "VICTORY_AMBIENT_AIR=1.5"
if not defined VICTORY_CACHE_DIR set "VICTORY_CACHE_DIR=%LOCALAPPDATA%\VIctory\cache-pareto"
if not defined VICTORY_TEXTURE_NO_WAIT set "VICTORY_TEXTURE_NO_WAIT=1"
if not defined VICTORY_RELAY set "VICTORY_RELAY=2"
if not defined VICTORY_WIDE_BACK set "VICTORY_WIDE_BACK=1"
if not defined VICTORY_CARD_REHASH_RUNS set "VICTORY_CARD_REHASH_RUNS=1"
if not defined VICTORY_JOIN_COPIES set "VICTORY_JOIN_COPIES=1"
if not defined VICTORY_INDIRECT_WINDOW set "VICTORY_INDIRECT_WINDOW=1"
if not defined VICTORY_RELAY_COPY set "VICTORY_RELAY_COPY=1"
if not defined VICTORY_CARD_STEPS set "VICTORY_CARD_STEPS=2"
if not defined VICTORY_INDIRECT_BATCH set "VICTORY_INDIRECT_BATCH=2"
if not defined VICTORY_MAP_THREADS set "VICTORY_MAP_THREADS=1"
if not defined VICTORY_CARD_AWAKE set "VICTORY_CARD_AWAKE=1"
if not defined VICTORY_SHOW_AFTER set "VICTORY_SHOW_AFTER=1"
if not defined VICTORY_AMBIENT set "VICTORY_AMBIENT=0.8"
if not defined VICTORY_THIN set "VICTORY_THIN=1"
if not defined VICTORY_JOIN_PARTS set "VICTORY_JOIN_PARTS=8"
if not defined VICTORY_UPLOAD_AHEAD set "VICTORY_UPLOAD_AHEAD=1"
if not defined VICTORY_LEVELS_ON_DEMAND set "VICTORY_LEVELS_ON_DEMAND=1"
if not defined VICTORY_CARD_BUFFERS_MIN_KB set "VICTORY_CARD_BUFFERS_MIN_KB=64"
if not defined VICTORY_LOG if not defined RUST_LOG set "VICTORY_LOG=warn"
if /i "%~1"=="rtx3060" (
  rem Largest mips left out so textures fit a 12 GB card (resource.rs).
  set "VICTORY_TEXTURE_LOW=2"
  rem Off: on an NVIDIA card this made GPU time worse (host.rs).
  set "VICTORY_CARD_FETCH_INFLIGHT=0"
  rem Off: the shortcut corrupts ray-tracing bounds (lower.rs).
  set "VICTORY_FIRSTLANE_OWN=0"
  rem Do not keep the AMD driver's never-finished compile hash (pipeline/library.rs).
  set "VICTORY_NO_COMPILE="
  rem Title memory kept on the card. Default is 4 GiB; 2 GiB leaves the rest of
  rem a 12 GB budget for textures and targets (shadow.rs).
  if not defined VICTORY_MEMORY_CARD_MB set "VICTORY_MEMORY_CARD_MB=2048"
  rem Per-frame texture staging for a local budget of at most 12 GiB (draw.rs).
  if not defined VICTORY_TEXTURE_STAGE_MB set "VICTORY_TEXTURE_STAGE_MB=192"
  rem Empty texture-pool blocks. Default can keep 4 GiB; 8 blocks are 512 MiB (pool.rs).
  if not defined VICTORY_POOL_SPARES set "VICTORY_POOL_SPARES=8"
  rem No raw-twin descriptor heap. That heap is 2^22 views; NVIDIA caps near
  rem 1,000,000 and hung on an index past the heap (descriptor/host.rs).
  if not defined VICTORY_RAW_VIEWS set "VICTORY_RAW_VIEWS=0"
)
"%runner%" run "%VICTORY_GAME_DIR%\game_scarlett_master.exe" --console series-s --execute
exit /b %errorlevel%
