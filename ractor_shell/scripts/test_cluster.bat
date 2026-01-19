@echo off

:: ============================================================
::  Ractor Shell - Raft Cluster Test Environment (Windows Batch)
:: ============================================================
::
:: Usage:
::   test_cluster.bat [options]
::
:: Options:
::   -release    Use optimized release build
::   -noshell    Don't start interactive shell
::   -nodes N    Number of cluster nodes (default: 3)
::   -debug      Enable debug-level Raft logging
::   -trace      Enable trace-level Raft logging
::
:: Examples:
::   test_cluster.bat                   # 3 nodes, debug build, with shell
::   test_cluster.bat -release          # 3 nodes, release build
::   test_cluster.bat -nodes 5          # 5 nodes
::   test_cluster.bat -noshell          # Start nodes without shell
::   test_cluster.bat -release -trace   # Release build with trace logging

:: Default configuration
set NODES=3
set PORT=9001
set COOKIE=secret_cookie
set RELEASE=0
set NOSHELL=0
set LOGLEVEL=info

:: Parse command line arguments
:parse_args
if "%~1"=="" goto main
if /i "%~1"=="-release" set RELEASE=1& shift & goto parse_args
if /i "%~1"=="-noshell" set NOSHELL=1& shift & goto parse_args
if /i "%~1"=="-debug" set LOGLEVEL=debug& shift & goto parse_args
if /i "%~1"=="-trace" set LOGLEVEL=trace& shift & goto parse_args
if /i "%~1"=="-nodes" set NODES=%~2& shift & shift & goto parse_args
if /i "%~1"=="-help" goto show_help
if /i "%~1"=="/?" goto show_help
echo Unknown option: %~1
goto show_help

:main
setlocal enabledelayedexpansion

:: Validate nodes
if %NODES% LSS 1 (
    echo Error: Must have at least 1 node
    exit /b 1
)

:: Determine script and repo directories
set SCRIPT_DIR=%~dp0
for %%i in ("%SCRIPT_DIR%..") do set PROJECT_DIR=%%~fi
for %%i in ("%PROJECT_DIR%\..") do set REPO_ROOT=%%~fi

:: Print header
echo ============================================================
echo   Ractor Shell - Raft Cluster Test Environment
echo ============================================================
echo.

:: Determine build mode
if %RELEASE%==1 (
    echo Building ractor_shell [release mode]...
    set BUILD_ARGS=build --example cluster_demo -p ractor_shell --release --quiet
    set TARGET_PATH=target\release\examples\cluster_demo.exe
) else (
    echo Building ractor_shell [debug mode]...
    set BUILD_ARGS=build --example cluster_demo -p ractor_shell --quiet
    set TARGET_PATH=target\debug\examples\cluster_demo.exe
)

:: Build the project
pushd %REPO_ROOT%
cargo %BUILD_ARGS%
if errorlevel 1 (
    echo Build failed!
    popd
    exit /b 1
)
popd
echo Build complete.
echo.

set EXECUTABLE=%REPO_ROOT%\%TARGET_PATH%

:: Set RUST_LOG based on log level
if "%LOGLEVEL%"=="trace" set RUST_LOG=cluster_demo::raft=trace
if "%LOGLEVEL%"=="debug" set RUST_LOG=cluster_demo::raft=debug
if "%LOGLEVEL%"=="info" set RUST_LOG=cluster_demo::raft=info

:: Start cluster nodes
echo Starting %NODES%-node Raft cluster...
if not "%LOGLEVEL%"=="info" echo   Log level: %LOGLEVEL%
echo.

:: Node letters for naming
set LETTERS=abcdefghijklmnopqrstuvwxyz

:: Start the first node (seed node)
set /a FIRST_PORT=%PORT%
set FIRST_NAME=node_a

echo   Starting %FIRST_NAME% on port %FIRST_PORT% [seed node]...
set LOG_FILE=%TEMP%\ractor_%FIRST_NAME%.log

:: Start first node in a minimized window
:: Using /B runs in background without new window, but we want to see node output
start "%FIRST_NAME%" /min "%EXECUTABLE%" node --port %FIRST_PORT% --name %FIRST_NAME% --cookie %COOKIE%

:: Give it time to start
timeout /t 2 /nobreak > nul
echo   * %FIRST_NAME% started

:: Start remaining nodes
set /a I=1
:start_nodes_loop
if %I% GEQ %NODES% goto done_starting_nodes

set /a NODE_PORT=%PORT%+%I%
set NODE_LETTER=!LETTERS:~%I%,1!
set NODE_NAME=node_!NODE_LETTER!

echo   Starting !NODE_NAME! on port !NODE_PORT! [connecting to %FIRST_NAME%]...

:: Start node in a minimized window
start "!NODE_NAME!" /min "%EXECUTABLE%" node --port !NODE_PORT! --name !NODE_NAME! --cookie %COOKIE% --peer 127.0.0.1:%FIRST_PORT%

timeout /t 1 /nobreak > nul
echo   * !NODE_NAME! started

set /a I+=1
goto start_nodes_loop

:done_starting_nodes

:: Wait for cluster to stabilize
echo.
echo Waiting for Raft leader election...
timeout /t 2 /nobreak > nul

echo.
echo Cluster started successfully.
echo.

:: Print cluster information
echo ------------------------------------------------------------
echo   Cluster Information
echo ------------------------------------------------------------
echo.

if %RELEASE%==1 (
    echo   Build: release [optimized]
) else (
    echo   Build: debug [use -release for lower CPU usage]
)
echo.

:: Print running nodes
echo   Nodes running:
set /a I=0
:print_nodes_loop
if %I% GEQ %NODES% goto done_print_nodes
set /a NODE_PORT=%PORT%+%I%
set NODE_LETTER=!LETTERS:~%I%,1!
set NODE_NAME=node_!NODE_LETTER!
echo     * !NODE_NAME! at 127.0.0.1:!NODE_PORT!
set /a I+=1
goto print_nodes_loop
:done_print_nodes
echo.

echo   Node output is shown in minimized console windows.
echo   Restore them to see node logs and debug output.
echo.

if not "%LOGLEVEL%"=="info" (
    echo   Trace logging enabled [RUST_LOG=%LOGLEVEL%]
    echo.
)

:: Branch based on shell mode
if %NOSHELL%==1 goto manual_mode

:: ============================================================
:: Interactive shell mode
:: ============================================================
echo ------------------------------------------------------------
echo   Starting Interactive Shell
echo ------------------------------------------------------------
echo.
echo   Quick commands to try:
echo.
echo     actors                              # List local actors
echo     pg members raft_cluster             # Show Raft cluster members
echo.
echo   Connect to a node and query Raft status:
echo.
echo     connect 127.0.0.1:%PORT%
echo     call raft_node IsLeader {}
echo     call raft_node GetLeader {}
echo     call raft_node GetStatus {}
echo     call raft_node GetPeers {}
echo     call raft_node StepDown {}          # Force leader to step down
echo.
echo   Check multiple nodes:
echo.

:: Print connect commands
set /a I=0
:print_connects_loop
if %I% GEQ %NODES% goto done_print_connects
set /a NODE_PORT=%PORT%+%I%
echo     connect 127.0.0.1:!NODE_PORT!
set /a I+=1
goto print_connects_loop
:done_print_connects

echo.
echo   Remote tracing [subscribe to events from a node]:
echo.
echo     trace remote 127.0.0.1:%PORT% *raft*    # Trace raft module events
echo     trace remote 127.0.0.1:%PORT% node_*    # Trace by actor name
echo     trace remote off                        # Stop remote tracing
echo.
echo ------------------------------------------------------------
echo.
echo   NOTE: When you exit the shell, close the node windows manually
echo         or run: taskkill /f /im cluster_demo.exe
echo.

:: Start the shell (blocks until exit)
"%EXECUTABLE%" shell

echo.
echo Stopping cluster nodes...
taskkill /f /im cluster_demo.exe > nul 2>&1
echo Done.
goto end

:: ============================================================
:: Manual testing mode (no shell)
:: ============================================================
:manual_mode
echo ------------------------------------------------------------
echo   Manual Testing Mode
echo ------------------------------------------------------------
echo.
echo   Start the shell manually with:
echo.
echo     "%EXECUTABLE%" shell
echo.
echo   Or connect directly to a node:
echo.
echo     "%EXECUTABLE%" shell --connect 127.0.0.1:%PORT%
echo.
echo   To stop all nodes:
echo.
echo     taskkill /f /im cluster_demo.exe
echo.
echo   Press any key to stop all nodes and exit.
echo   [Node windows will remain open until you close them]
echo.

:: Wait for user input
pause
taskkill /f /im cluster_demo.exe > nul 2>&1
goto end

:end
exit /b 0

:: ============================================================
:: Help
:: ============================================================
:show_help
echo.
echo Usage: test_cluster.bat [options]
echo.
echo Options:
echo   -release    Use optimized release build [faster, lower CPU]
echo   -noshell    Don't start interactive shell [for manual testing]
echo   -nodes N    Number of cluster nodes [default: 3]
echo   -debug      Enable debug-level Raft logging to files
echo   -trace      Enable trace-level Raft logging [verbose]
echo   -help       Show this help message
echo.
echo Examples:
echo   test_cluster.bat                    Start 3-node cluster + shell
echo   test_cluster.bat -release           Use release build
echo   test_cluster.bat -nodes 5           Start 5-node cluster
echo   test_cluster.bat -noshell           Start cluster only
echo   test_cluster.bat -release -trace    Release with trace logging
echo.
echo Raft Commands [use via shell 'call' command]:
echo   IsLeader {}   - Check if node is the leader
echo   GetLeader {}  - Get current leader name
echo   GetStatus {}  - Get full node status
echo   GetPeers {}   - List connected peers
echo   StepDown {}   - Force leader to step down
echo.
exit /b 0
