#!/usr/bin/env bash
# ScaryDB Launcher for Linux/macOS

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Ensure data directory exists
mkdir -p data

# Detect binary name
BINARY="./scarydb"
if [[ ! -f "$BINARY" ]]; then
    BINARY="./scarydb.exe"
fi

if [[ ! -f "$BINARY" ]]; then
    echo "Error: ScaryDB binary not found in $SCRIPT_DIR"
    exit 1
fi

# Make binary executable
chmod +x "$BINARY"

# Parse arguments
MODE="${1:-standalone}"

case "$MODE" in
    standalone|--standalone)
        echo "Starting ScaryDB in standalone mode (server + client)..."
        exec "$BINARY" standalone
        ;;
    server|--server)
        echo "Starting ScaryDB server..."
        exec "$BINARY" server
        ;;
    client|--client)
        echo "Starting ScaryDB client..."
        exec "$BINARY" client
        ;;
    log-read|--log-read)
        if [[ -z "${2:-}" ]]; then
            echo "Usage: $0 log-read <path_to_operations.log>"
            exit 1
        fi
        exec "$BINARY" log-read "$2"
        ;;
    *)
        echo "Usage: $0 [standalone|server|client|log-read <path>]"
        echo ""
        echo "Modes:"
        echo "  standalone  - Run server and client together (default)"
        echo "  server      - Run only the database server"
        echo "  client      - Run only the REPL client"
        echo "  log-read    - Read binary WAL log file"
        exit 1
        ;;
esac
