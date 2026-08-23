$ErrorActionPreference = 'Stop'

$packageName = 'scarydb'
$version = '0.1.0'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$installDir = "$env:ProgramFiles\ScaryDB"
$url64 = "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v$version/scurydb-$version-x86_64-pc-windows-gnu.zip"
$checksum64 = "REPLACE_WITH_ACTUAL_SHA256"

$packageArgs = @{
  packageName   = $packageName
  unzipLocation = $toolsDir
  url64bit      = $url64
  checksum64    = $checksum64
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# Move to Program Files
if (Test-Path $installDir) {
  Remove-Item $installDir -Recurse -Force
}
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Copy-Item "$toolsDir\scarydb.exe" "$installDir\scarydb.exe"
Copy-Item "$toolsDir\config.json" "$installDir\config.json"

# Add to PATH
Install-ChocolateyPath $installDir -PathType Machine

# Install as Windows Service
$serviceName = 'ScaryDB'
$serviceDisplayName = 'ScaryDB Server'
$serviceDescription = 'High-performance in-memory hierarchical database server'

try {
  if (Get-Service $serviceName -ErrorAction SilentlyContinue) {
    Stop-Service $serviceName
    sc.exe delete $serviceName
  }
} catch {}

New-Service -Name $serviceName -BinaryPathName "$installDir\scarydb.exe server" -DisplayName $serviceDisplayName -Description $serviceDescription -StartupType Automatic
Start-Service $serviceName

Write-Host "ScaryDB installed and started as Windows Service: $serviceName"
Write-Host "Binary location: $installDir\scarydb.exe"
Write-Host "Config location: $installDir\config.json"