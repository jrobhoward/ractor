<#
.SYNOPSIS
    Start a Raft cluster for testing ractor_shell on Windows

.DESCRIPTION
    This script starts a 3-node cluster with Raft leader election.
    Nodes automatically connect and elect a leader.

.PARAMETER Nodes
    Number of cluster nodes to start (default: 3)

.PARAMETER Port
    Starting port number (default: 9001)

.PARAMETER Cookie
    Cluster authentication cookie (default: secret_cookie)

.PARAMETER NoShell
    Don't start the interactive shell (for manual testing)

.PARAMETER Release
    Use optimized release build (faster, lower CPU usage)

.PARAMETER Debug
    Enable debug-level Raft logging to files

.PARAMETER Trace
    Enable trace-level Raft logging (verbose)

.EXAMPLE
    .\test_cluster.ps1
    Start 3-node Raft cluster + shell (debug build)

.EXAMPLE
    .\test_cluster.ps1 -Release
    Start with optimized release build

.EXAMPLE
    .\test_cluster.ps1 -Nodes 5
    Start 5-node cluster + shell

.EXAMPLE
    .\test_cluster.ps1 -NoShell
    Start cluster only (for manual testing)

.EXAMPLE
    .\test_cluster.ps1 -Trace
    Start with verbose tracing to log files

.NOTES
    Raft Commands (use via shell 'call' command):
      IsLeader {}   - Check if node is the leader
      GetLeader {}  - Get current leader name
      GetStatus {}  - Get full node status
      GetPeers {}   - List connected peers
      StepDown {}   - Force leader to step down (triggers election)

    Remote Tracing:
      trace remote 127.0.0.1:9001 *raft*  - Subscribe to raft events
      trace remote 127.0.0.1:9001 node_*  - Subscribe by actor name
      trace remote off                    - Stop remote tracing

.LINK
    Basic usage (3 nodes + shell, debug build):
      .\ractor_shell\scripts\test_cluster.ps1

    With release build (recommended for lower CPU usage):
      .\ractor_shell\scripts\test_cluster.ps1 -Release

    Custom number of nodes:
      .\ractor_shell\scripts\test_cluster.ps1 -Nodes 5

    Enable debug/trace logging:
      .\ractor_shell\scripts\test_cluster.ps1 -Debug
      .\ractor_shell\scripts\test_cluster.ps1 -Trace

    Start nodes without shell (for manual testing):
      .\ractor_shell\scripts\test_cluster.ps1 -NoShell

    View help:
      Get-Help .\ractor_shell\scripts\test_cluster.ps1 -Full
#>

[CmdletBinding()]
param(
    [int]$Nodes = 3,
    [int]$Port = 9001,
    [string]$Cookie = "secret_cookie",
    [switch]$NoShell,
    [switch]$Release,
    [switch]$Debug,
    [switch]$Trace
)

# Script configuration
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$RepoRoot = Split-Path -Parent $ProjectDir

# Track background jobs for cleanup
$script:NodeJobs = @()

# Cleanup function
function Stop-ClusterNodes {
    Write-Host "`nShutting down cluster nodes..." -ForegroundColor Yellow

    foreach ($job in $script:NodeJobs) {
        if ($job -and (Get-Job -Id $job.Id -ErrorAction SilentlyContinue)) {
            Stop-Job -Id $job.Id -ErrorAction SilentlyContinue
            Remove-Job -Id $job.Id -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Host "Cluster stopped." -ForegroundColor Green
}

# Register cleanup on exit
$null = Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action {
    Stop-ClusterNodes
}

# Handle Ctrl+C
$null = Register-ObjectEvent -InputObject ([Console]) -EventName CancelKeyPress -Action {
    Stop-ClusterNodes
    [Environment]::Exit(0)
}

# Validate minimum nodes
if ($Nodes -lt 1) {
    Write-Host "Error: Must have at least 1 node" -ForegroundColor Red
    exit 1
}

# Print header
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  Ractor Shell - Raft Cluster Test Environment" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host ""

# Determine build mode and cargo target
if ($Release) {
    Write-Host "Building ractor_shell (release mode)..." -ForegroundColor Yellow
    $BuildType = "release"
    $CargoArgs = @("build", "--example", "cluster_demo", "-p", "ractor_shell", "--release", "--quiet")
    $TargetPath = "target\release\examples\cluster_demo.exe"
} else {
    Write-Host "Building ractor_shell (debug mode)..." -ForegroundColor Yellow
    $BuildType = "debug"
    $CargoArgs = @("build", "--example", "cluster_demo", "-p", "ractor_shell", "--quiet")
    $TargetPath = "target\debug\examples\cluster_demo.exe"
}

# Build the project
Push-Location $RepoRoot
try {
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed"
    }
    Write-Host "Build complete." -ForegroundColor Green
    Write-Host ""
} finally {
    Pop-Location
}

# Set up RUST_LOG based on log level
if ($Trace) {
    $env:RUST_LOG = "cluster_demo::raft=trace"
    $LogLevel = "trace"
} elseif ($Debug) {
    $env:RUST_LOG = "cluster_demo::raft=debug"
    $LogLevel = "debug"
} else {
    $env:RUST_LOG = "cluster_demo::raft=info"
    $LogLevel = "info"
}

# Start cluster nodes
Write-Host "Starting $Nodes-node Raft cluster..." -ForegroundColor Yellow
if ($LogLevel -ne "info") {
    Write-Host "  Log level: $LogLevel (RUST_LOG=$env:RUST_LOG)" -ForegroundColor Cyan
}
Write-Host ""

# Generate node names (node_a, node_b, node_c, ...)
$NodeNames = @()
$NodeAddrs = @()
for ($i = 1; $i -le $Nodes; $i++) {
    $letter = [char](96 + $i)  # 97=a, 98=b, etc.
    $NodeNames += "node_$letter"
}

# Start the first node (seed node)
$FirstPort = $Port
$FirstName = $NodeNames[0]
$NodeAddrs += "127.0.0.1:$FirstPort"

Write-Host "  Starting $FirstName on port $FirstPort (seed node)..." -ForegroundColor Cyan

$LogFile = "$env:TEMP\ractor_${FirstName}.log"
$CargoTarget = Join-Path $RepoRoot $TargetPath

# Start first node as background job
$NodeJob = Start-Job -ScriptBlock {
    param($Executable, $Port, $Name, $Cookie, $LogFile)
    & $Executable node --port $Port --name $Name --cookie $Cookie *>&1 | Out-File -FilePath $LogFile -Encoding UTF8
} -ArgumentList $CargoTarget, $FirstPort, $FirstName, $Cookie, $LogFile

$script:NodeJobs += $NodeJob

# Give the first node time to start
Start-Sleep -Seconds 1

# Check if it's still running
if ($NodeJob.State -ne "Running") {
    Write-Host "  Failed to start $FirstName. Check $LogFile for details." -ForegroundColor Red
    Receive-Job -Job $NodeJob
    Stop-ClusterNodes
    exit 1
}

Write-Host "  ✓ $FirstName started (Job ID: $($NodeJob.Id))" -ForegroundColor Green

# Start remaining nodes
for ($i = 2; $i -le $Nodes; $i++) {
    $NodePort = $Port + $i - 1
    $NodeName = $NodeNames[$i - 1]
    $NodeAddrs += "127.0.0.1:$NodePort"

    Write-Host "  Starting $NodeName on port $NodePort (connecting to $FirstName)..." -ForegroundColor Cyan

    $LogFile = "$env:TEMP\ractor_${NodeName}.log"

    # Start node as background job
    $NodeJob = Start-Job -ScriptBlock {
        param($Executable, $Port, $Name, $Cookie, $PeerAddr, $LogFile)
        & $Executable node --port $Port --name $Name --cookie $Cookie --peer $PeerAddr *>&1 | Out-File -FilePath $LogFile -Encoding UTF8
    } -ArgumentList $CargoTarget, $NodePort, $NodeName, $Cookie, "127.0.0.1:$FirstPort", $LogFile

    $script:NodeJobs += $NodeJob

    # Give the node time to start
    Start-Sleep -Milliseconds 500

    # Check if it's still running
    if ($NodeJob.State -ne "Running") {
        Write-Host "  Failed to start $NodeName. Check $LogFile for details." -ForegroundColor Red
        Receive-Job -Job $NodeJob
        Stop-ClusterNodes
        exit 1
    }

    Write-Host "  ✓ $NodeName started (Job ID: $($NodeJob.Id))" -ForegroundColor Green
}

# Wait for cluster to stabilize and elect a leader
Write-Host ""
Write-Host "Waiting for Raft leader election..." -ForegroundColor Yellow
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "Cluster started successfully." -ForegroundColor Green
Write-Host ""

# Print cluster information
Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
Write-Host "  Cluster Information" -ForegroundColor Cyan
Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
Write-Host ""

if ($Release) {
    Write-Host "  Build: release (optimized)" -ForegroundColor Green
} else {
    Write-Host "  Build: debug (use -Release for lower CPU usage)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "  Nodes running:"
for ($i = 1; $i -le $Nodes; $i++) {
    $NodePort = $Port + $i - 1
    $NodeName = $NodeNames[$i - 1]
    Write-Host "    • $NodeName at 127.0.0.1:$NodePort" -ForegroundColor Green
}

Write-Host ""
Write-Host "  Log files:"
for ($i = 1; $i -le $Nodes; $i++) {
    $NodeName = $NodeNames[$i - 1]
    Write-Host "    $env:TEMP\ractor_${NodeName}.log"
}
Write-Host ""

if ($LogLevel -ne "info") {
    Write-Host "  Trace logging enabled! Watch logs with:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "    Get-Content $env:TEMP\ractor_node_*.log -Wait -Tail 20" -ForegroundColor Green
    Write-Host ""
}

if (-not $NoShell) {
    Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host "  Starting Interactive Shell" -ForegroundColor Cyan
    Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Quick commands to try:"
    Write-Host ""
    Write-Host "    actors" -ForegroundColor Green -NoNewline
    Write-Host "                              # List local actors"
    Write-Host "    pg members raft_cluster" -ForegroundColor Green -NoNewline
    Write-Host "             # Show Raft cluster members"
    Write-Host ""
    Write-Host "  Connect to a node and query Raft status:"
    Write-Host ""
    Write-Host "    connect 127.0.0.1:$Port" -ForegroundColor Green
    Write-Host "    call raft_node IsLeader {}" -ForegroundColor Green
    Write-Host "    call raft_node GetLeader {}" -ForegroundColor Green
    Write-Host "    call raft_node GetStatus {}" -ForegroundColor Green
    Write-Host "    call raft_node GetPeers {}" -ForegroundColor Green
    Write-Host "    call raft_node StepDown {}" -ForegroundColor Green -NoNewline
    Write-Host "              # Force leader to step down"
    Write-Host ""
    Write-Host "  Check multiple nodes:"
    Write-Host ""
    foreach ($addr in $NodeAddrs) {
        Write-Host "    connect $addr" -ForegroundColor Green
    }
    Write-Host ""
    Write-Host "  Remote tracing (subscribe to events from a node):"
    Write-Host ""
    Write-Host "    trace remote 127.0.0.1:$Port *raft*" -ForegroundColor Green -NoNewline
    Write-Host "    # Trace raft module events"
    Write-Host "    trace remote 127.0.0.1:$Port node_*" -ForegroundColor Green -NoNewline
    Write-Host "    # Trace by actor name"
    Write-Host "    trace remote off" -ForegroundColor Green -NoNewline
    Write-Host "                        # Stop remote tracing"
    Write-Host ""
    Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host ""

    # Start the shell (this blocks until the user exits)
    try {
        & $CargoTarget shell
    } finally {
        Stop-ClusterNodes
    }
} else {
    Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host "  Manual Testing Mode" -ForegroundColor Cyan
    Write-Host "------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Start the shell manually with:"
    Write-Host ""
    Write-Host "    $CargoTarget shell" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Or connect directly to a node:"
    Write-Host ""
    Write-Host "    $CargoTarget shell --connect 127.0.0.1:$Port" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Press Ctrl+C to stop all nodes."
    Write-Host ""

    # Wait for Ctrl+C
    try {
        while ($true) {
            Start-Sleep -Seconds 1

            # Check if any jobs have failed
            foreach ($job in $script:NodeJobs) {
                if ($job.State -eq "Failed" -or $job.State -eq "Stopped") {
                    Write-Host "Node job $($job.Id) has failed/stopped!" -ForegroundColor Red
                    Receive-Job -Job $job
                }
            }
        }
    } finally {
        Stop-ClusterNodes
    }
}
