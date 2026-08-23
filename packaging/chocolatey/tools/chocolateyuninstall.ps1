$ErrorActionPreference = 'Stop'

$packageName = 'scarydb'
$installDir = "$env:ProgramFiles\ScaryDB"
$serviceName = 'ScaryDB'

# Stop and remove service
try {
  if (Get-Service $serviceName -ErrorAction SilentlyContinue) {
    Stop-Service $serviceName
    sc.exe delete $serviceName
    Write-Host "Removed Windows Service: $serviceName"
  }
} catch {}

# Remove from PATH
$pathToRemove = $installDir
$machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
if ($machinePath -like "*$pathToRemove*") {
  $newPath = $machinePath -replace [regex]::Escape($pathToRemove) + ';?', ''
  [Environment]::SetEnvironmentVariable('Path', $newPath, 'Machine')
  Write-Host "Removed $pathToRemove from system PATH"
}

# Remove installation directory
if (Test-Path $installDir) {
  Remove-Item $installDir -Recurse -Force
  Write-Host "Removed installation directory: $installDir"
}

Write-Host "ScaryDB uninstalled successfully"