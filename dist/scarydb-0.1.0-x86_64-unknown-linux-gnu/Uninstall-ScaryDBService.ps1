<#
.SYNOPSIS
    Uninstalls ScaryDB Windows Service

.NOTES
    Run as Administrator
#>

param (
    [string]$ServiceName = "ScaryDB",
    [string]$InstallPath = "C:\Program Files\ScaryDB"
)

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges."
    exit 1
}

# Stop and remove service
if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName
    Write-Host "Service '$ServiceName' removed." -ForegroundColor Green
}

# Remove installation directory
if (Test-Path $InstallPath) {
    Remove-Item -Path $InstallPath -Recurse -Force
    Write-Host "Installation directory removed." -ForegroundColor Green
}
