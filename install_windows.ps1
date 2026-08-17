<#
.SYNOPSIS
    ScaryDB Windows Installation Script
    Installs ScaryDB as a Windows Service

.DESCRIPTION
    This script installs ScaryDB binary and optionally registers it as a Windows Service.
    Requires Administrator privileges.

.NOTES
    Run as Administrator:
    PowerShell -ExecutionPolicy Bypass -File install_windows.ps1

    Optional parameters:
    -InstallPath "C:\Program Files\ScaryDB"
    -ServiceName "ScaryDB"
    -NoService (install binary only)
    -NoStart (don't start service after install)
    -Force (overwrite existing installation)
#>

param (
    [string]$InstallPath = "C:\Program Files\ScaryDB",
    [string]$ServiceName = "ScaryDB",
    [string]$DisplayName = "ScaryDB Database Server",
    [string]$Description = "High-performance in-memory hierarchical database",
    [switch]$NoService,
    [switch]$NoStart,
    [switch]$Force
)

# Colors for output
$Red = [ConsoleColor]::Red
$Green = [ConsoleColor]::Green
$Yellow = [ConsoleColor]::Yellow
$Blue = [ConsoleColor]::Cyan
$Default = [ConsoleColor]::Gray

function Write-Log {
    param([string]$Message, [ConsoleColor]$Color = $Default)
    $originalColor = $Host.UI.RawUI.ForegroundColor
    $Host.UI.RawUI.ForegroundColor = $Color
    Write-Host $Message
    $Host.UI.RawUI.ForegroundColor = $originalColor
}

function Write-Info { Write-Log "[INFO] $($args[0])" $Blue }
function Write-Success { Write-Log "[SUCCESS] $($args[0])" $Green }
function Write-Warning { Write-Log "[WARNING] $($args[0])" $Yellow }
function Write-Error { Write-Log "[ERROR] $($args[0])" $Red }

# Check for Administrator privileges
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges."
    Write-Error "Please run PowerShell as Administrator and try again."
    exit 1
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$BinaryName = "scarydb.exe"
$ConfigFile = "config.json"
$BinarySource = Join-Path $ScriptDir $BinaryName
$ConfigSource = Join-Path $ScriptDir $ConfigFile

Write-Info "ScaryDB Windows Installation"
Write-Info "Install path: $InstallPath"
Write-Info "Service name: $ServiceName"

# Check for existing installation
if (Test-Path $InstallPath -and -not $Force) {
    Write-Error "Installation directory already exists: $InstallPath"
    Write-Info "Use -Force to overwrite or specify different path with -InstallPath"
    exit 1
}

# Check for binary
if (-not (Test-Path $BinarySource)) {
    Write-Error "Binary not found: $BinarySource"
    Write-Info "Run build.sh first or place scarydb.exe in the same directory as this script"
    exit 1
}

# Remove existing installation if forced
if (Test-Path $InstallPath -and $Force) {
    Write-Warning "Removing existing installation..."
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        sc.exe delete $ServiceName | Out-Null
    }
    Remove-Item -Path $InstallPath -Recurse -Force -ErrorAction SilentlyContinue
}

# Create installation directory
Write-Info "Creating installation directory..."
New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $InstallPath "data") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $InstallPath "logs") -Force | Out-Null

# Copy binary
Write-Info "Copying binary..."
Copy-Item -Path $BinarySource -Destination (Join-Path $InstallPath $BinaryName) -Force

# Copy config
if (Test-Path $ConfigSource) {
    Write-Info "Copying configuration..."
    Copy-Item -Path $ConfigSource -Destination (Join-Path $InstallPath $ConfigFile) -Force
} else {
    Write-Warning "Config file not found, creating default..."
    $defaultConfig = @{
        server = @{ workers = 1 }
        storage = @{ data_dir = "./data"; checkpoint_interval_ops = 10000 }
        memory = @{ max_memory_kb = 0 }
        network = @{ host = "127.0.0.1"; port = 6379 }
        metadata = @{ version = "0.1.0"; startup_time = (Get-Date).ToString("o") }
    } | ConvertTo-Json -Depth 5
    $defaultConfig | Set-Content -Path (Join-Path $InstallPath $ConfigFile) -Encoding UTF8
}

# Set permissions (read/execute for Users, full for Administrators/System)
$acl = Get-Acl $InstallPath
$acl.SetAccessRuleProtection($true, $true)
$ruleUsers = New-Object Security.AccessControl.FileSystemAccessRule("Users","ReadAndExecute","ContainerInherit,ObjectInherit","None","Allow")
$ruleAdmins = New-Object Security.AccessControl.FileSystemAccessRule("Administrators","FullControl","ContainerInherit,ObjectInherit","None","Allow")
$ruleSystem = New-Object Security.AccessControl.FileSystemAccessRule("SYSTEM","FullControl","ContainerInherit,ObjectInherit","None","Allow")
$acl.AddAccessRule($ruleUsers)
$acl.AddAccessRule($ruleAdmins)
$acl.AddAccessRule($ruleSystem)
Set-Acl -Path $InstallPath -AclObject $acl

# Install Windows Service
if (-not $NoService) {
    Write-Info "Installing Windows Service..."
    
    # Check if service already exists
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
        Write-Warning "Service '$ServiceName' already exists. Removing..."
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        sc.exe delete $ServiceName | Out-Null
        Start-Sleep -Seconds 2
    }
    
    # Create service
    $binaryPath = Join-Path $InstallPath $BinaryName
    $serviceArgs = "server"
    
    try {
        New-Service -Name $ServiceName `
                    -DisplayName $DisplayName `
                    -Description $Description `
                    -BinaryPathName "`"$binaryPath`" $serviceArgs" `
                    -StartupType Automatic `
                    -ErrorAction Stop | Out-Null
        
        Write-Success "Service '$ServiceName' created successfully"
        
        # Set service recovery options
        sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/60000 | Out-Null
        
        # Set service description
        sc.exe description $ServiceName $Description | Out-Null
        
        if (-not $NoStart) {
            Write-Info "Starting service..."
            Start-Service -Name $ServiceName -ErrorAction Stop
            Start-Sleep -Seconds 3
            
            $serviceStatus = (Get-Service -Name $ServiceName).Status
            if ($serviceStatus -eq 'Running') {
                Write-Success "Service started successfully!"
                Write-Info "Check status: Get-Service $ServiceName"
                Write-Info "View logs: Get-WinEvent -LogName Application -ProviderName $ServiceName"
            } else {
                Write-Error "Service failed to start. Status: $serviceStatus"
                Write-Info "Check Event Viewer for details"
                exit 1
            }
        } else {
            Write-Info "Service installed but not started (-NoStart flag)"
            Write-Info "Start manually: Start-Service -Name $ServiceName"
        }
    } catch {
        Write-Error "Failed to create service: $_"
        exit 1
    }
} else {
    Write-Info "Skipping service installation (-NoService flag)"
}

# Add to PATH (machine-wide)
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
if ($currentPath -notlike "*$InstallPath*") {
    Write-Info "Adding to system PATH..."
    [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$InstallPath", "Machine")
    Write-Success "Added $InstallPath to system PATH"
    Write-Warning "Restart your terminal or run 'refreshenv' to update PATH"
} else {
    Write-Info "Install path already in system PATH"
}

Write-Success "Installation complete!"
Write-Info ""
Write-Info "Installation summary:"
Write-Info "  Binary:     $InstallPath\$BinaryName"
Write-Info "  Config:     $InstallPath\$ConfigFile"
Write-Info "  Data dir:   $InstallPath\data\"
Write-Info "  Logs dir:   $InstallPath\logs\"
if (-not $NoService) {
    Write-Info "  Service:    $ServiceName (Windows Service)"
}
Write-Info ""
Write-Info "Quick start:"
Write-Info "  scarydb standalone    # Run server + client"
Write-Info "  scarydb server        # Run server only"
Write-Info "  scarydb client        # Run client only"
Write-Info ""
if (-not $NoService -and -not $NoStart) {
    Write-Info "Service is running. Test connection:"
    Write-Info "  scarydb client"
}