Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,WindowsBase
$out = @{
  ts = (Get-Date).ToUniversalTime().ToString('o')
  vantage = 'acer'
  processes = @()
  windows = @()
  controls = @()
  tree_left_pane = @()
}
Write-Host '=== spacedesk processes ==='
$procs = Get-Process -Name 'spacedesk*' -ErrorAction SilentlyContinue
foreach ($p in $procs) {
  $out.processes += @{ pid = $p.Id; name = $p.ProcessName; main_title = $p.MainWindowTitle }
  Write-Host ('  pid=' + $p.Id + ' name=' + $p.ProcessName + ' title="' + $p.MainWindowTitle + '"')
}

if ($procs.Count -eq 0) { Write-Host 'NO SPACEDESK PROCESSES'; $out | ConvertTo-Json -Depth 6 -Compress; exit 0 }

$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Window)
$allWindows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)

Write-Host ''
Write-Host '=== windows owned by spacedesk processes ==='
$spacedeskPids = @($procs | ForEach-Object { $_.Id })
foreach ($w in $allWindows) {
  $owningPid = 0
  try { $owningPid = $w.Current.ProcessId } catch { continue }
  if (-not ($spacedeskPids -contains $owningPid)) { continue }
  $title = ''
  try { $title = $w.Current.Name } catch {}
  $className = ''
  try { $className = $w.Current.ClassName } catch {}
  Write-Host ('  pid=' + $owningPid + ' class=' + $className + ' title="' + $title + '"')
  $out.windows += @{ pid = $owningPid; class = $className; title = $title }

  Write-Host '  --- walking tree ---'
  $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
  $stack = New-Object System.Collections.Stack
  $stack.Push(@{ el = $w; depth = 0 })
  $count = 0
  while ($stack.Count -gt 0 -and $count -lt 200) {
    $item = $stack.Pop()
    $el = $item.el
    $depth = $item.depth
    $count++
    try {
      $aid = ''; try { $aid = $el.Current.AutomationId } catch {}
      $name = ''; try { $name = $el.Current.Name } catch {}
      $ctype = ''; try { $ctype = $el.Current.ControlType.ProgrammaticName } catch {}
      $patterns = @()
      try {
        $supported = $el.GetSupportedPatterns()
        foreach ($p in $supported) { $patterns += $p.ProgrammaticName }
      } catch {}
      if ($aid -or $name) {
        $row = @{ depth = $depth; aid = $aid; name = $name; ctype = $ctype; patterns = ($patterns -join ',') }
        $out.controls += $row
        $indent = '    ' + ('  ' * $depth)
        Write-Host ($indent + '[' + $ctype + '] aid="' + $aid + '" name="' + $name + '" patterns=' + ($patterns -join ','))
      }
      $child = $walker.GetFirstChild($el)
      while ($child -ne $null) {
        $stack.Push(@{ el = $child; depth = ($depth + 1) })
        $child = $walker.GetNextSibling($child)
      }
    } catch {}
  }
}

$json = $out | ConvertTo-Json -Depth 6 -Compress
$jsonPath = 'C:/asolaria-acer/federation-remake-1024/tools/omniscrcpy/broadcasts/looks/spacedesk-deep-walk-acer-' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH-mm-ssZ') + '.json'
[System.IO.File]::WriteAllText($jsonPath, $json)
Write-Host ''
Write-Host ('=== JSON written: ' + $jsonPath + ' ===')
Write-Host ('=== controls inventoried: ' + $out.controls.Count + ' ===')
