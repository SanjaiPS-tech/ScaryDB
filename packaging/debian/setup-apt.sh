#!/bin/bash
# GPG key setup for APT repository
# Run as root to add the ScaryDB repository key

set -euo pipefail

GPG_KEY_URL="https://apt.scarydb.io/scarydb.gpg"
KEYRING_PATH="/usr/share/keyrings/scarydb-archive-keyring.gpg"

# Download and install GPG key
curl -fsSL "$GPG_KEY_URL" | gpg --dearmor -o "$KEYRING_PATH"
chmod 644 "$KEYRING_PATH"

# Add repository
echo "deb [arch=$(dpkg --print-architecture) signed-by=$KEYRING_PATH] https://apt.scarydb.io stable main" > /etc/apt/sources.list.d/scarydb.list

# Update package list
apt-get update

echo "ScaryDB repository added. Install with: apt-get install scarydb"