<#
.SYNOPSIS
    Installs ScaryDB as a Windows Service

.DESCRIPTION
    Creates and starts a Windows Service for ScaryDB server mode.
    Requires Administrator privileges.

.NOTES
    Run as Administrator: PowerShell -ExecutionPolicy Bypass -File Install-ScaryDBService.ps1
#>

param (
    [string]$InstallPath = "C:\Program Files\ScaryDB",
    [string]$ServiceName = "ScaryDB",
    [string]$DisplayName = "ScaryDB Database Server",
    [string]$Description = "High-performance in-memory hierarchical database"
)

# Check for admin rights
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges. Please run as Administrator."
    exit 1
}

# Create installation directory
if (-not (Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
}

# Copy files (assumes running from package directory)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Copy-Item "$ScriptDir\scarydb.exe" "$InstallPath\" -Force
Copy-Item "$ScriptDir\config.json" "$InstallPath\" -Force
if (-not (Test-Path "$InstallPath\data")) {
    New-Item -ItemType Directory -Path "$InstallPath\data" | Out-Null
}

# Create service
$ServiceParams = @{
    Name        = $ServiceName
    DisplayName = $DisplayName
    Description = $Description
    BinaryPathName = "$InstallPath\scarydb.exe server"
    StartupType = 'Automatic'
    Credential  = (Get-Credential -Message "Enter service account credentials (or cancel for LocalSystem)")
}

try {
    New-Service @ServiceParams -ErrorAction Stop
    Write-Host "Service '$ServiceName' created successfully." -ForegroundColor Green
    
    # Start service
    Start-Service -Name $ServiceName -ErrorAction Stop
    Write-Host "Service '$ServiceName' started successfully." -ForegroundColor Green
} catch {
    Write-Error "Failed to create/start service: $_"
    exit 1
}
