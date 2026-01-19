#!/usr/bin/env bash
#
# test_cluster.sh - Start a Raft cluster for testing ractor_shell
#
# This script starts a 3-node cluster with Raft leader election.
# Nodes automatically connect and elect a leader.
#
# Usage:
#   ./scripts/test_cluster.sh              # Start 3 nodes + shell (default)
#   ./scripts/test_cluster.sh --release    # Start with optimized release build
#   ./scripts/test_cluster.sh --nodes 5    # Start 5 nodes + shell
#   ./scripts/test_cluster.sh --no-shell   # Start nodes only (for manual testing)
#   ./scripts/test_cluster.sh --trace      # Enable trace-level logging
#   ./scripts/test_cluster.sh --debug      # Enable debug-level logging
#
# Raft Commands (via shell using typed RPC):
#   call raft_node IsLeader {}   - Check if node is leader
#   call raft_node GetLeader {}  - Get current leader name
#   call raft_node GetStatus {}  - Get full status
#   call raft_node GetPeers {}   - List connected peers
#   call raft_node StepDown {}   - Force leader to step down (triggers new election)
#
# Remote Tracing:
#   trace remote 127.0.0.1:9001 *raft*  - Subscribe to raft events
#   trace remote 127.0.0.1:9001 node_*  - Subscribe by actor name
#   trace remote off                    - Stop remote tracing
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/.." && pwd)"

# Default configuration
NUM_NODES=3
START_PORT=9001
COOKIE="secret_cookie"
START_SHELL=true
LOG_LEVEL="info"  # info, debug, or trace
RELEASE_MODE=false
PIDS=()

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  ${BOLD}Ractor Shell - Raft Cluster Test Environment${NC}"
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --release      Use optimized release build (faster, lower CPU usage)"
    echo "  --nodes N      Number of cluster nodes to start (default: 3)"
    echo "  --port PORT    Starting port number (default: 9001)"
    echo "  --cookie STR   Cluster authentication cookie (default: secret_cookie)"
    echo "  --no-shell     Don't start the interactive shell"
    echo "  --debug        Enable debug-level Raft logging to files"
    echo "  --trace        Enable trace-level Raft logging (verbose)"
    echo "  --help         Show this help message"
    echo
    echo "Examples:"
    echo "  $0                    # Start 3-node Raft cluster + shell (debug build)"
    echo "  $0 --release          # Start with optimized release build"
    echo "  $0 --nodes 5          # Start 5-node cluster + shell"
    echo "  $0 --no-shell         # Start cluster only (for manual testing)"
    echo "  $0 --trace            # Start with verbose tracing to log files"
    echo
    echo "Raft Commands (use via shell 'call' command):"
    echo "  IsLeader {}   - Check if node is the leader"
    echo "  GetLeader {}  - Get current leader name"
    echo "  GetStatus {}  - Get full node status"
    echo "  GetPeers {}   - List connected peers"
    echo "  StepDown {}   - Force leader to step down (triggers election)"
    echo
    echo "Remote Tracing:"
    echo "  trace remote 127.0.0.1:9001 *raft*  - Subscribe to raft events"
    echo "  trace remote 127.0.0.1:9001 node_*  - Subscribe by actor name"
    echo "  trace remote off                    - Stop remote tracing"
    echo
}

cleanup() {
    echo
    echo -e "${YELLOW}Shutting down cluster nodes...${NC}"
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    # Wait a moment for graceful shutdown
    sleep 1
    # Force kill any remaining
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null || true
        fi
    done
    echo -e "${GREEN}Cluster stopped.${NC}"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --nodes)
            NUM_NODES="$2"
            shift 2
            ;;
        --port)
            START_PORT="$2"
            shift 2
            ;;
        --cookie)
            COOKIE="$2"
            shift 2
            ;;
        --no-shell)
            START_SHELL=false
            shift
            ;;
        --release)
            RELEASE_MODE=true
            shift
            ;;
        --debug)
            LOG_LEVEL="debug"
            shift
            ;;
        --trace)
            LOG_LEVEL="trace"
            shift
            ;;
        --help)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

# Validate minimum nodes
if [ "$NUM_NODES" -lt 1 ]; then
    echo -e "${RED}Error: Must have at least 1 node${NC}"
    exit 1
fi

# Set up cleanup trap
trap cleanup EXIT INT TERM

print_header

# Build the project first
if [ "$RELEASE_MODE" = true ]; then
    echo -e "${YELLOW}Building ractor_shell (release mode)...${NC}"
    cd "$REPO_ROOT"
    cargo build --example cluster_demo -p ractor_shell --release --quiet
    CARGO_TARGET="target/release/examples/cluster_demo"
    BUILD_TYPE="release"
else
    echo -e "${YELLOW}Building ractor_shell (debug mode)...${NC}"
    cd "$REPO_ROOT"
    cargo build --example cluster_demo -p ractor_shell --quiet
    CARGO_TARGET="target/debug/examples/cluster_demo"
    BUILD_TYPE="debug"
fi
echo -e "${GREEN}Build complete.${NC}"
echo

# Set up RUST_LOG based on log level
# Note: We intentionally exclude ractor_cluster debug logs as they're extremely
# verbose (logs every network SEND/RECEIVE). Use --cluster-debug if you need them.
# The raft module is now in the example code (cluster_demo::raft).
case $LOG_LEVEL in
    trace)
        export RUST_LOG="cluster_demo::raft=trace"
        ;;
    debug)
        export RUST_LOG="cluster_demo::raft=debug"
        ;;
    *)
        export RUST_LOG="cluster_demo::raft=info"
        ;;
esac

# Start cluster nodes
echo -e "${YELLOW}Starting $NUM_NODES-node Raft cluster...${NC}"
if [ "$LOG_LEVEL" != "info" ]; then
    echo -e "  Log level: ${CYAN}$LOG_LEVEL${NC} (RUST_LOG=$RUST_LOG)"
fi
echo

NODE_ADDRS=()
NODE_NAMES=()

# Generate node names (node_a, node_b, node_c, ...)
for i in $(seq 1 $NUM_NODES); do
    # Convert number to letter (1=a, 2=b, etc.)
    letter=$(printf "\\$(printf '%03o' $((96 + i)))")
    NODE_NAMES+=("node_$letter")
done

# Start the first node (no peer connection needed)
FIRST_PORT=$START_PORT
FIRST_NAME="${NODE_NAMES[0]}"
NODE_ADDRS+=("127.0.0.1:$FIRST_PORT")

echo -e "  Starting ${CYAN}$FIRST_NAME${NC} on port ${CYAN}$FIRST_PORT${NC} (seed node)..."

LOG_FILE="/tmp/ractor_${FIRST_NAME}.log"
"$REPO_ROOT/$CARGO_TARGET" node \
    --port "$FIRST_PORT" \
    --name "$FIRST_NAME" \
    --cookie "$COOKIE" \
    > "$LOG_FILE" 2>&1 &

NODE_PID=$!
PIDS+=($NODE_PID)

# Give the first node time to start and bind
sleep 1

# Check if it's still running
if ! kill -0 "$NODE_PID" 2>/dev/null; then
    echo -e "${RED}  Failed to start $FIRST_NAME. Check $LOG_FILE for details.${NC}"
    cat "$LOG_FILE"
    exit 1
fi

echo -e "  ${GREEN}✓${NC} $FIRST_NAME started (PID: $NODE_PID)"

# Start remaining nodes, connecting to the first node
# (transitive mode will automatically connect them to each other)
for i in $(seq 2 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    NODE_NAME="${NODE_NAMES[$((i-1))]}"
    NODE_ADDRS+=("127.0.0.1:$PORT")

    echo -e "  Starting ${CYAN}$NODE_NAME${NC} on port ${CYAN}$PORT${NC} (connecting to $FIRST_NAME)..."

    LOG_FILE="/tmp/ractor_${NODE_NAME}.log"
    "$REPO_ROOT/$CARGO_TARGET" node \
        --port "$PORT" \
        --name "$NODE_NAME" \
        --cookie "$COOKIE" \
        --peer "127.0.0.1:$FIRST_PORT" \
        > "$LOG_FILE" 2>&1 &

    NODE_PID=$!
    PIDS+=($NODE_PID)

    # Give the node time to start and connect
    sleep 0.5

    # Check if it's still running
    if ! kill -0 "$NODE_PID" 2>/dev/null; then
        echo -e "${RED}  Failed to start $NODE_NAME. Check $LOG_FILE for details.${NC}"
        exit 1
    fi

    echo -e "  ${GREEN}✓${NC} $NODE_NAME started (PID: $NODE_PID)"
done

# Wait for cluster to stabilize and elect a leader
echo
echo -e "${YELLOW}Waiting for Raft leader election...${NC}"
sleep 2

echo
echo -e "${GREEN}Cluster started successfully.${NC}"
echo

# Print cluster information
echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
echo -e "${CYAN}  ${BOLD}Cluster Information${NC}"
echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
echo
if [ "$RELEASE_MODE" = true ]; then
    echo -e "  Build: ${GREEN}release${NC} (optimized)"
else
    echo -e "  Build: ${YELLOW}debug${NC} (use --release for lower CPU usage)"
fi
echo
echo "  Nodes running:"
for i in $(seq 1 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    NAME="${NODE_NAMES[$((i-1))]}"
    echo -e "    ${GREEN}•${NC} $NAME at 127.0.0.1:$PORT"
done
echo
echo "  Log files:"
for i in $(seq 1 $NUM_NODES); do
    NAME="${NODE_NAMES[$((i-1))]}"
    echo "    /tmp/ractor_${NAME}.log"
done
echo
if [ "$LOG_LEVEL" != "info" ]; then
    echo -e "  ${YELLOW}Trace logging enabled!${NC} Watch logs with:"
    echo
    echo -e "    ${GREEN}tail -f /tmp/ractor_node_*.log${NC}"
    echo
fi

if [ "$START_SHELL" = true ]; then
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo -e "${CYAN}  ${BOLD}Starting Interactive Shell${NC}"
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo
    echo "  Quick commands to try:"
    echo
    echo -e "    ${GREEN}actors${NC}                              # List local actors"
    echo -e "    ${GREEN}pg members raft_cluster${NC}             # Show Raft cluster members"
    echo
    echo "  Connect to a node and query Raft status:"
    echo
    echo -e "    ${GREEN}connect 127.0.0.1:$START_PORT${NC}"
    echo -e "    ${GREEN}call raft_node IsLeader {}${NC}"
    echo -e "    ${GREEN}call raft_node GetLeader {}${NC}"
    echo -e "    ${GREEN}call raft_node GetStatus {}${NC}"
    echo -e "    ${GREEN}call raft_node GetPeers {}${NC}"
    echo -e "    ${GREEN}call raft_node StepDown {}${NC}              # Force leader to step down"
    echo
    echo "  Check multiple nodes:"
    echo
    for addr in "${NODE_ADDRS[@]}"; do
        echo -e "    ${GREEN}connect $addr${NC}"
    done
    echo
    echo "  Remote tracing (subscribe to events from a node):"
    echo
    echo -e "    ${GREEN}trace remote 127.0.0.1:$START_PORT *raft*${NC}    # Trace raft module events"
    echo -e "    ${GREEN}trace remote 127.0.0.1:$START_PORT node_*${NC}    # Trace by actor name"
    echo -e "    ${GREEN}trace remote off${NC}                        # Stop remote tracing"
    echo
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo

    # Start the shell (this blocks until the user exits)
    "$REPO_ROOT/$CARGO_TARGET" shell
else
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo -e "${CYAN}  ${BOLD}Manual Testing Mode${NC}"
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo
    echo "  Start the shell manually with:"
    echo
    echo -e "    ${GREEN}$REPO_ROOT/$CARGO_TARGET shell${NC}"
    echo
    echo "  Or connect directly to a node:"
    echo
    echo -e "    ${GREEN}$REPO_ROOT/$CARGO_TARGET shell --connect 127.0.0.1:$START_PORT${NC}"
    echo
    echo "  Press Ctrl+C to stop all nodes."
    echo

    # Wait for Ctrl+C
    while true; do
        sleep 1
    done
fi
