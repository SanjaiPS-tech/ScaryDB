@echo off
REM =============================================================================
REM ScaryDB Windows Installation Script (Batch)
REM Installs ScaryDB as a Windows Service
REM =============================================================================

REM Run as Administrator required

setlocal enabledelayedexpansion

REM Configuration
set PROJECT_NAME=scarydb
set INSTALL_PATH=C:\Program Files\ScaryDB
set SERVICE_NAME=ScaryDB
set DISPLAY_NAME=ScaryDB Database Server
set DESCRIPTION=High-performance in-memory hierarchical database
set NO_SERVICE=false
set NO_START=false
set FORCE=false

REM Colors (using ANSI escape codes - works in Windows 10+)
for /f %%a in ('echo prompt $E ^| cmd') do set "ESC=%%a"
set RED=%ESC%[91m
set GREEN=%ESC%[92m
set YELLOW=%ESC%[93m
set BLUE=%ESC%[94m
set NC=%ESC%[0m

:parse_args
if "%~1"=="" goto :args_done
if /i "%~1"=="-h" goto :show_help
if /i "%~1"=="--help" goto :show_help
if /i "%~1"=="-d" set "INSTALL_PATH=%~2" & shift & shift & goto :parse_args
if /i "%~1"=="--dir" set "INSTALL_PATH=%~2" & shift & shift & goto :parse_args
if /i "%~1"=="--no-service" set "NO_SERVICE=true" & shift & goto :parse_args
if /i "%~1"=="--no-start" set "NO_START=true" & shift & goto :parse_args
if /i "%~1"=="--force" set "FORCE=true" & shift & goto :parse_args
echo Unknown option: %~1
goto :show_help
:args_done

:show_help
echo ScaryDB Windows Installation Script (Batch)
echo.
echo Usage: %~nx0 [OPTIONS]
echo.
echo OPTIONS:
echo   -h, --help           Show this help
echo   -d, --dir PATH       Installation directory (default: C:\Program Files\ScaryDB)
echo   --no-service         Don't install Windows Service
echo   --no-start           Don't start service after installation
echo   --force              Overwrite existing installation
echo.
echo EXAMPLES:
echo   %~nx0
echo   %~nx0 --dir D:\ScaryDB
echo   %~nx0 --no-service
echo.
goto :eof

:log_info
echo %BLUE%[INFO]%NC% %*
goto :eof

:log_success
echo %GREEN%[SUCCESS]%NC% %*
goto :eof

:log_warning
echo %YELLOW%[WARNING]%NC% %*
goto :eof

:log_error
echo %RED%[ERROR]%NC% %*
goto :eof

REM Check for Administrator privileges
net session >nul 2>&1
if %errorLevel% neq 0 (
    call :log_error This script requires Administrator privileges.
    call :log_error Please run Command Prompt as Administrator and try again.
    exit /b 1
)

set SCRIPT_DIR=%~dp0
set BINARY_NAME=scarydb.exe
set CONFIG_FILE=config.json
set BINARY_SOURCE=%SCRIPT_DIR%%BINARY_NAME%
set CONFIG_SOURCE=%SCRIPT_DIR%%CONFIG_FILE%

call :log_info ScaryDB Windows Installation
call :log_info Install path: %INSTALL_PATH%
call :log_info Service name: %SERVICE_NAME%

REM Check for existing installation
if exist "%INSTALL_PATH%" (
    if "%FORCE%"=="false" (
        call :log_error Installation directory already exists: %INSTALL_PATH%
        call :log_info Use --force to overwrite or specify different path with -d
        exit /b 1
    )
)

REM Check for binary
if not exist "%BINARY_SOURCE%" (
    call :log_error Binary not found: %BINARY_SOURCE%
    call :log_info Run build.sh first or place scarydb.exe in the same directory as this script
    exit /b 1
)

REM Remove existing installation if forced
if exist "%INSTALL_PATH%" if "%FORCE%"=="true" (
    call :log_warning Removing existing installation...
    sc query "%SERVICE_NAME%" >nul 2>&1
    if %errorLevel% equ 0 (
        net stop "%SERVICE_NAME%" >nul 2>&1
        sc delete "%SERVICE_NAME%" >nul 2>&1
    )
    rmdir /s /q "%INSTALL_PATH%" 2>nul
)

REM Create installation directory
call :log_info Creating installation directory...
mkdir "%INSTALL_PATH%" 2>nul
mkdir "%INSTALL_PATH%\data" 2>nul
mkdir "%INSTALL_PATH%\logs" 2>nul

REM Copy binary
call :log_info Copying binary...
copy /y "%BINARY_SOURCE%" "%INSTALL_PATH%\%BINARY_NAME%" >nul

REM Copy config
if exist "%CONFIG_SOURCE%" (
    call :log_info Copying configuration...
    copy /y "%CONFIG_SOURCE%" "%INSTALL_PATH%\%CONFIG_FILE%" >nul
) else (
    call :log_warning Config file not found, creating default...
    echo { > "%INSTALL_PATH%\%CONFIG_FILE%"
    echo   "server": { "workers": 1 }, >> "%INSTALL_PATH%\%CONFIG_FILE%"
    echo   "storage": { "data_dir": "./data", "checkpoint_interval_ops": 10000 }, >> "%INSTALL_PATH%\%CONFIG_FILE%"
    echo   "memory": { "max_memory_kb": 0 }, >> "%INSTALL_PATH%\%CONFIG_FILE%"
    echo   "network": { "host": "127.0.0.1", "port": 6379 }, >> "%INSTALL_PATH%\%CONFIG_FILE%"
    echo   "metadata": { "version": "0.1.0", "startup_time": "%DATE%T%TIME%Z" } >> "%INSTALL_PATH%\%CONFIG_FILE%"
    echo } >> "%INSTALL_PATH%\%CONFIG_FILE%"
)

REM Install Windows Service
if "%NO_SERVICE%"=="false" (
    call :log_info Installing Windows Service...
    
    REM Check if service already exists
    sc query "%SERVICE_NAME%" >nul 2>&1
    if %errorLevel% equ 0 (
        call :log_warning Service '%SERVICE_NAME%' already exists. Removing...
        net stop "%SERVICE_NAME%" >nul 2>&1
        sc delete "%SERVICE_NAME%" >nul 2>&1
        timeout /t 2 >nul
    )
    
    set BINARY_PATH=%INSTALL_PATH%\%BINARY_NAME%
    set SERVICE_ARGS=server
    
    sc create "%SERVICE_NAME%" binPath= "\"%BINARY_PATH%\" %SERVICE_ARGS%" DisplayName= "%DISPLAY_NAME%" start= auto >nul
    if %errorLevel% neq 0 (
        call :log_error Failed to create service
        exit /b 1
    )
    
    call :log_success Service '%SERVICE_NAME%' created successfully
    
    REM Set service recovery options
    sc failure "%SERVICE_NAME%" reset= 86400 actions= restart/5000/restart/10000/restart/60000 >nul
    
    REM Set service description
    sc description "%SERVICE_NAME%" "%DESCRIPTION%" >nul
    
    if "%NO_START%"=="false" (
        call :log_info Starting service...
        net start "%SERVICE_NAME%" >nul
        if %errorLevel% neq 0 (
            call :log_error Service failed to start
            call :log_info Check Event Viewer for details
            exit /b 1
        )
        timeout /t 3 >nul
        sc query "%SERVICE_NAME%" | find "RUNNING" >nul
        if %errorLevel% equ 0 (
            call :log_success Service started successfully!
            call :log_info Check status: sc query %SERVICE_NAME%
        ) else (
            call :log_error Service failed to start properly
            exit /b 1
        )
    ) else (
        call :log_info Service installed but not started (--no-start flag)
        call :log_info Start manually: net start %SERVICE_NAME%
    )
) else (
    call :log_info Skipping service installation (--no-service flag)
)

REM Add to system PATH
set CURRENT_PATH=
for /f "tokens=2*" %%a in ('reg query "HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment" /v PATH 2^>nul') do set "CURRENT_PATH=%%b"
echo "%CURRENT_PATH%" | find "%INSTALL_PATH%" >nul
if %errorLevel% neq 0 (
    call :log_info Adding to system PATH...
    setx PATH "%CURRENT_PATH%;%INSTALL_PATH%" /M >nul
    call :log_success Added %INSTALL_PATH% to system PATH
    call :log_warning Restart your terminal or run 'refreshenv' to update PATH
) else (
    call :log_info Install path already in system PATH
)

call :log_success Installation complete!
call :log_info.
call :log_info Installation summary:
call :log_info   Binary:     %INSTALL_PATH%\%BINARY_NAME%
call :log_info   Config:     %INSTALL_PATH%\%CONFIG_FILE%
call :log_info   Data dir:   %INSTALL_PATH%\data\
call :log_info   Logs dir:   %INSTALL_PATH%\logs\
if "%NO_SERVICE%"=="false" call :log_info   Service:    %SERVICE_NAME% (Windows Service)
call :log_info.
call :log_info Quick start:
call :log_info   scarydb standalone    ^# Run server + client
call :log_info   scarydb server        ^# Run server only
call :log_info   scarydb client        ^# Run client only
call :log_info.
if "%NO_SERVICE%"=="false" if "%NO_START%"=="false" (
    call :log_info Service is running. Test connection:
    call :log_info   scarydb client
)

exit /b 0