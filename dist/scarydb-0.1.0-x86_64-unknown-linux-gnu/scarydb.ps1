# ScaryDB Launcher for Windows (PowerShell)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

# Ensure data directory exists
if (-not (Test-Path "data")) {
    New-Item -ItemType Directory -Path "data" | Out-Null
}

$Binary = "scarydb.exe"
if (-not (Test-Path $Binary)) {
    Write-Error "ScaryDB binary not found in $ScriptDir"
    exit 1
}

$Mode = $args[0] ?? "standalone"

switch -Wildcard ($Mode.ToLower()) {
    "standalone" {
        Write-Host "Starting ScaryDB in standalone mode (server + client)..."
        & $Binary standalone
    }
    "server" {
        Write-Host "Starting ScaryDB server..."
        & $Binary server
    }
    "client" {
        Write-Host "Starting ScaryDB client..."
        & $Binary client
    }
    "log-read" {
        if ($args.Count -lt 2) {
            Write-Host "Usage: .\scarydb.ps1 log-read <path_to_operations.log>"
            exit 1
        }
        & $Binary log-read $args[1]
    }
    default {
        Write-Host "Usage: .\scarydb.ps1 [standalone|server|client|log-read <path>]"
        Write-Host ""
        Write-Host "Modes:"
        Write-Host "  standalone  - Run server and client together (default)"
        Write-Host "  server      - Run only the database server"
        Write-Host "  client      - Run only the REPL client"
        Write-Host "  log-read    - Read binary WAL log file"
        exit 1
    }
}
