# Checks a PC before the first run of the rtx3060 preset. It reads only, starts nothing and
# changes nothing. Run it in the window you will play from, after setting the paths:
#   $env:VICTORY_GAME_DIR = 'D:\pixel\victory\game'; $env:VICTORY_CACHE_DIR = 'D:\pixel\victory\cache'
#   scripts\preflight.ps1
# Each line is OK, WARN (it may run, but read the line) or FAIL (it will not run).
param(
    [string]$GameDir = $env:VICTORY_GAME_DIR,
    [string]$CacheDir = $env:VICTORY_CACHE_DIR
)
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$script:fails = 0
$script:warns = 0
function Say([string]$what, [string]$text) {
    if ($what -eq 'FAIL') { $script:fails++ }
    if ($what -eq 'WARN') { $script:warns++ }
    $colour = @{ OK = 'Green'; WARN = 'Yellow'; FAIL = 'Red' }[$what]
    Write-Host ('{0,-4} {1}' -f $what, $text) -ForegroundColor $colour
}
function Inside([string]$path) {
    $full = [IO.Path]::GetFullPath($path).TrimEnd('\') + '\'
    $full.StartsWith($root.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)
}
function Gb([double]$bytes) { '{0:N1} GB' -f ($bytes / 1GB) }

# The build.
$exe = Join-Path $root 'target\release\victory.exe'
if (-not (Test-Path $exe)) {
    Say FAIL "no ${exe}: cargo build --release --locked -p victory"
} else {
    $built = (Get-Item $exe).LastWriteTime
    $head = $null
    try { $head = [DateTimeOffset]::FromUnixTimeSeconds([int64](git -C $root log -1 --format=%ct 2>$null)).LocalDateTime } catch { }
    $dirty = @(git -C $root status --porcelain --untracked-files=no 2>$null | Where-Object { $_ -match '\.(rs|toml)$' -or $_ -match 'Cargo\.lock' })
    if ($head -and $built -lt $head) {
        Say WARN "victory.exe ($built) is older than the checked-out commit ($head): build again"
    } elseif ($dirty.Count -gt 0) {
        Say WARN "victory.exe built $built; Rust sources have uncommitted changes, so it may not match them"
    } else {
        Say OK "victory.exe built $built, after commit $(git -C $root log -1 --format=%h 2>$null)"
    }
}

# The dump.
if (-not $GameDir) {
    Say FAIL 'VICTORY_GAME_DIR is not set'
} elseif (-not (Test-Path (Join-Path $GameDir 'game_scarlett_master.exe'))) {
    Say FAIL "$GameDir has no game_scarlett_master.exe (still copying?)"
} else {
    if (Inside $GameDir) { Say FAIL "$GameDir is inside the checkout; the tools refuse it" }
    $files = Get-ChildItem $GameDir -Recurse -File -ErrorAction SilentlyContinue
    $size = ($files | Measure-Object Length -Sum).Sum
    Say OK ('{0}: {1:N0} files, {2}' -f $GameDir, $files.Count, (Gb $size))
    $partial = @($files | Where-Object { $_.Length -eq 0 })
    if ($partial.Count -gt 0) { Say WARN "$($partial.Count) empty files in the dump, e.g. $($partial[0].FullName): normal for some dumps, but check that the copy finished" }
}

# The cache: shader translations and the driver's pipelines.
if (-not $CacheDir) {
    Say WARN "VICTORY_CACHE_DIR is not set: the launcher uses $env:LOCALAPPDATA\VIctory\cache-pareto"
    $CacheDir = Join-Path $env:LOCALAPPDATA 'VIctory\cache-pareto'
}
if (Inside $CacheDir) { Say FAIL "$CacheDir is inside the checkout; the tools refuse it" }
$shaders = Join-Path $CacheDir 'shaders'
$count = @(Get-ChildItem $shaders -Recurse -File -ErrorAction SilentlyContinue).Count
if ($count -eq 0) {
    Say WARN "no translated shaders in $shaders yet: set the preset's switches (docs\rtx3060-first-run.md), then target\release\victory.exe shaders"
} else {
    Say OK "$count files of translated shaders in $shaders"
}
$drive = (Split-Path -Qualifier ([IO.Path]::GetFullPath($CacheDir))).TrimEnd(':')
$free = (Get-PSDrive $drive -ErrorAction SilentlyContinue).Free
if ($free -and $free -lt 20GB) { Say WARN "only $(Gb $free) free on ${drive}: for the cache" } elseif ($free) { Say OK "$(Gb $free) free on ${drive}:" }

# The card and its driver.
$cards = @(Get-CimInstance Win32_VideoController | Where-Object { $_.Name -match 'NVIDIA|AMD|Radeon|Intel\(R\) Arc' })
foreach ($c in $cards) { Say OK "$($c.Name), driver $($c.DriverVersion)" }
if (-not ($cards | Where-Object { $_.Name -match 'NVIDIA' })) { Say WARN 'no NVIDIA card found; the rtx3060 preset is for one' }

# Windows' GPU timeout: a long frame on a slower card is reset as a device removal.
$gd = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -ErrorAction SilentlyContinue
$level = $gd.TdrLevel; $delay = $gd.TdrDelay
if ($level -eq 0) { Say WARN 'TdrLevel is 0: a GPU hang freezes the PC instead of ending the run' }
elseif ($level -eq 1) { Say WARN 'TdrLevel is 1: a GPU timeout is a blue screen' }
if (-not $delay -or $delay -lt 8) {
    $shown = 'unset (2 s)'
    if ($delay) { $shown = "$delay s" }
    Say WARN "TdrDelay $shown. If runs end with DXGI_ERROR_DEVICE_HUNG and no page fault, set the DWORD TdrDelay to 10 under HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers and restart"
} else {
    Say OK "TdrDelay $delay s"
}

# Memory: the title commits tens of GB; Windows ends allocations when the commit limit is reached.
$os = Get-CimInstance Win32_OperatingSystem
$ram = $os.TotalVisibleMemorySize * 1KB
$limit = $os.TotalVirtualMemorySize * 1KB
$left = $os.FreeVirtualMemory * 1KB
$text = 'RAM {0}, commit limit {1} (RAM plus page file), {2} of it free now' -f (Gb $ram), (Gb $limit), (Gb $left)
if ($left -lt 40GB) { Say WARN "$text. Runs have needed 26-46 GB of private memory; close programs or grow the page file" } else { Say OK $text }

# Things that make frame times unreadable.
$overlays = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^(obs64|obs32|MSIAfterburner|RTSS|Discord)$' } | ForEach-Object ProcessName | Sort-Object -Unique)
if ($overlays.Count -gt 0) { Say WARN "running: $($overlays -join ', '): overlays and capture change frame times; close them for measured runs" }

Write-Host ''
if ($script:fails -gt 0) { Write-Host "$script:fails FAIL, $script:warns WARN: fix the FAIL lines first." -ForegroundColor Red; exit 1 }
Write-Host "No FAIL, $script:warns WARN. Next: docs\rtx3060-first-run.md, the checklist." -ForegroundColor Green
