# Sums up a VICTORY_STATS file: frame time, card memory, private memory, and the
# counts that point at flicker and late textures. One row of the file is one second.
#   scripts\stats-summary.ps1 D:\pixel\victory\stats\run.csv
#   scripts\stats-summary.ps1 run.csv -From 120          # leave out the first two minutes (menus, loading)
#   scripts\stats-summary.ps1 run.csv -MinDraws 1000     # only seconds in the world
param(
    [Parameter(Mandatory)][string]$Path,
    [double]$From = 0,
    [double]$MinDraws = 0
)
$rows = @(Import-Csv $Path | Where-Object { [double]$_.seconds -ge $From -and [double]$_.draws -ge $MinDraws })
if ($rows.Count -eq 0) { Write-Host "No rows in $Path with seconds >= $From and draws >= $MinDraws."; return }

function Num([object[]]$r, [string]$name) { , @($r | ForEach-Object { [double]$_.$name }) }
function Pct([double[]]$v, [double]$p) {
    $s = @($v | Sort-Object)
    $i = [math]::Min($s.Count - 1, [math]::Max(0, [math]::Ceiling($p / 100 * $s.Count) - 1))
    $s[$i]
}
function Most([double[]]$v) { ($v | Measure-Object -Maximum).Maximum }
function Least([double[]]$v) { ($v | Measure-Object -Minimum).Minimum }
function Mean([double[]]$v) { ($v | Measure-Object -Average).Average }

$ms = Num $rows 'frame_ms'
$worst = Num $rows 'worst_ms'
$long = @($worst | Where-Object { $_ -gt 100 }).Count
$first = $rows[0]; $last = $rows[$rows.Count - 1]

'{0} seconds ({1:N0} to {2:N0} s, frames {3} to {4})' -f $rows.Count, [double]$first.seconds, [double]$last.seconds, $first.frame, $last.frame
'frame time, the seconds'' means: median {0:N1} ms, p95 {1:N1}, p99 {2:N1}; fps median {3:N1}' -f (Pct $ms 50), (Pct $ms 95), (Pct $ms 99), (Pct (Num $rows 'fps') 50)
'longest single frame {0:N0} ms; {1} of {2} seconds had a frame over 100 ms' -f (Most $worst), $long, $rows.Count
'submitting thread: preparing median {0:N1} ms a frame, waiting for the card median {1:N1}' -f (Pct (Num $rows 'preparing_ms') 50), (Pct (Num $rows 'gpu_wait_ms') 50)
'draws median {0:N0} a frame; dropped for want of a pipeline: mean {1:N2} a frame, {2} seconds with any' -f (Pct (Num $rows 'draws') 50), (Mean (Num $rows 'dropped_no_pipeline')), @($rows | Where-Object { [double]$_.dropped_no_pipeline -gt 0 }).Count
'indirect records run empty (list budget): mean {0:N2} a frame, {1} seconds with any' -f (Mean (Num $rows 'indirect_empty')), @($rows | Where-Object { [double]$_.indirect_empty -gt 0 }).Count
'card memory: peak {0:N0} MB used of a budget of {1:N0} to {2:N0} MB; shared (PC) memory peak {3:N0} MB' -f (Most (Num $rows 'vram_used_mb')), (Least (Num $rows 'vram_budget_mb')), (Most (Num $rows 'vram_budget_mb')), (Most (Num $rows 'shared_used_mb'))
'private memory: peak {0:N0} MB, last {1:N0} MB; working set peak {2:N0} MB; least commit left {3:N0} MB' -f (Most (Num $rows 'private_mb')), [double]$last.private_mb, (Most (Num $rows 'working_mb')), (Least (Num $rows 'commit_left_mb'))
'texture queue: up to {0:N0} slots waiting (median {1:N0}); it found the card full {2} times in these seconds' -f (Most (Num $rows 'textures_waiting')), (Pct (Num $rows 'textures_waiting') 50), ([double]$last.texture_queue_full_total - [double]$first.texture_queue_full_total)
$switches = "$Path.switches.txt"
if (Test-Path $switches) { "switches: $switches" }
