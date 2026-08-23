#!/bin/bash
# GPG key setup for DNF repository
# Run as root to add the ScaryDB repository

set -euo pipefail

GPG_KEY_URL="https://dnf.scarydb.io/scarydb.gpg"
REPO_FILE="/etc/yum.repos.d/scarydb.repo"

# Import GPG key
rpm --import "$GPG_KEY_URL"

# Add repository
cat > "$REPO_FILE" <<EOF
[scarydb]
name=ScaryDB Repository
baseurl=https://dnf.scarydb.io/stable/\$basearch/
enabled=1
gpgcheck=1
gpgkey=$GPG_KEY_URL
repo_gpgcheck=1
metadata_expire=1h
EOF

# Update package cache
dnf makecache

echo "ScaryDB repository added. Install with: dnf install scarydb"