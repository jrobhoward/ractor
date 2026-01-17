#!/usr/bin/env bash
#
# test_cluster.sh - Start a local cluster for testing ractor_shell
#
# This script starts multiple cluster nodes and the shell for interactive testing.
#
# Usage:
#   ./scripts/test_cluster.sh           # Start 2 nodes + shell
#   ./scripts/test_cluster.sh --nodes 3 # Start 3 nodes + shell
#   ./scripts/test_cluster.sh --no-shell # Start nodes only (for manual shell testing)
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/.." && pwd)"

# Default configuration
NUM_NODES=2
START_PORT=9002
COOKIE="secret_cookie"
START_SHELL=true
PIDS=()

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  Ractor Shell - Local Cluster Test Environment${NC}"
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --nodes N      Number of cluster nodes to start (default: 2)"
    echo "  --port PORT    Starting port number (default: 9002)"
    echo "  --cookie STR   Cluster authentication cookie (default: secret_cookie)"
    echo "  --no-shell     Don't start the interactive shell"
    echo "  --help         Show this help message"
    echo
    echo "Examples:"
    echo "  $0                    # Start 2 nodes + shell"
    echo "  $0 --nodes 3          # Start 3 nodes + shell"
    echo "  $0 --no-shell         # Start nodes only"
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

# Set up cleanup trap
trap cleanup EXIT INT TERM

print_header

# Build the project first
echo -e "${YELLOW}Building ractor_shell...${NC}"
cd "$REPO_ROOT"
cargo build --example cluster_node -p ractor_shell --quiet
cargo build --example demo -p ractor_shell --quiet
echo -e "${GREEN}Build complete.${NC}"
echo

# Start cluster nodes
echo -e "${YELLOW}Starting $NUM_NODES cluster nodes...${NC}"
echo

NODE_ADDRS=()
for i in $(seq 1 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    NODE_NAME="node_$i"
    NODE_ADDRS+=("127.0.0.1:$PORT")

    echo -e "  Starting ${CYAN}$NODE_NAME${NC} on port ${CYAN}$PORT${NC}..."

    # Start node in background, redirect output to a log file
    LOG_FILE="/tmp/ractor_node_${NODE_NAME}.log"
    cargo run --example cluster_node -p ractor_shell --quiet -- \
        --port "$PORT" \
        --name "$NODE_NAME" \
        --cookie "$COOKIE" \
        > "$LOG_FILE" 2>&1 &

    NODE_PID=$!
    PIDS+=($NODE_PID)

    # Give the node a moment to start
    sleep 0.5

    # Check if it's still running
    if ! kill -0 "$NODE_PID" 2>/dev/null; then
        echo -e "${RED}  Failed to start $NODE_NAME. Check $LOG_FILE for details.${NC}"
        exit 1
    fi

    echo -e "  ${GREEN}✓${NC} $NODE_NAME started (PID: $NODE_PID, log: $LOG_FILE)"
done

echo
echo -e "${GREEN}All nodes started successfully.${NC}"
echo

# Print connection instructions
echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
echo -e "${CYAN}  Cluster Information${NC}"
echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
echo
echo "  Nodes running:"
for i in $(seq 1 $NUM_NODES); do
    PORT=$((START_PORT + i - 1))
    echo -e "    ${GREEN}•${NC} node_$i at 127.0.0.1:$PORT"
done
echo
echo "  Log files:"
for i in $(seq 1 $NUM_NODES); do
    echo "    /tmp/ractor_node_node_$i.log"
done
echo

if [ "$START_SHELL" = true ]; then
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo -e "${CYAN}  Starting Interactive Shell${NC}"
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo
    echo "  Quick commands to try:"
    echo
    for addr in "${NODE_ADDRS[@]}"; do
        echo -e "    ${GREEN}connect $addr${NC}"
    done
    echo -e "    ${GREEN}nodes${NC}              # List connected nodes"
    echo -e "    ${GREEN}use 127.0.0.1:$START_PORT${NC}    # Switch to node context"
    echo -e "    ${GREEN}registry${NC}           # Show actors on current node"
    echo -e "    ${GREEN}pg members workers${NC} # Show worker actors"
    echo -e "    ${GREEN}cluster${NC}            # Show cluster topology"
    echo
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo

    # Start the shell (this blocks until the user exits)
    cargo run --example demo -p ractor_shell --quiet
else
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo -e "${CYAN}  Manual Testing Mode${NC}"
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
    echo
    echo "  Start the shell manually with:"
    echo
    echo -e "    ${GREEN}cargo run --example demo -p ractor_shell${NC}"
    echo
    echo "  Or use the shell binary directly:"
    echo
    echo -e "    ${GREEN}cargo run --bin ractor-shell${NC}"
    echo
    echo "  Press Ctrl+C to stop all nodes."
    echo

    # Wait for Ctrl+C
    while true; do
        sleep 1
    done
fi
