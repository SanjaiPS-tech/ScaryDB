@echo off
REM ScaryDB Launcher for Windows

set SCRIPT_DIR=%~dp0
cd /d "%SCRIPT_DIR%"

REM Ensure data directory exists
if not exist data mkdir data

set BINARY=scarydb.exe
if not exist %BINARY% (
    echo Error: ScaryDB binary not found in %SCRIPT_DIR%
    exit /b 1
)

set MODE=%1
if "%MODE%"=="" set MODE=standalone

if /i "%MODE%"=="standalone" (
    echo Starting ScaryDB in standalone mode (server + client)...
    %BINARY% standalone
) else if /i "%MODE%"=="server" (
    echo Starting ScaryDB server...
    %BINARY% server
) else if /i "%MODE%"=="client" (
    echo Starting ScaryDB client...
    %BINARY% client
) else if /i "%MODE%"=="log-read" (
    if "%~2"=="" (
        echo Usage: %0 log-read ^<path_to_operations.log^>
        exit /b 1
    )
    %BINARY% log-read %2
) else (
    echo Usage: %0 [standalone^|server^|client^|log-read ^<path^>]
    echo.
    echo Modes:
    echo   standalone  - Run server and client together (default)
    echo   server      - Run only the database server
    echo   client      - Run only the REPL client
    echo   log-read    - Read binary WAL log file
    exit /b 1
)
