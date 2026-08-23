#!/bin/bash
# Build script for Debian package
# Run this on a Debian/Ubuntu system to create .deb packages

set -euo pipefail

PACKAGE_NAME="scarydb"
VERSION="0.1.0"
ARCH="amd64"  # or arm64
MAINTAINER="SanjaiPS-tech <maintainers@scarydb.io>"
DESCRIPTION="High-performance, in-memory, actor-based hierarchical database"

# Build the binary first
cargo build --release --target x86_64-unknown-linux-gnu

# Create package structure
DEB_ROOT=$(mktemp -d)
DEB_DEBIAN="$DEB_ROOT/DEBIAN"
DEB_BIN="$DEB_ROOT/usr/bin"
DEB_CONFIG="$DEB_ROOT/etc/scarydb"
DEB_LIB="$DEB_ROOT/var/lib/scarydb"
DEB_LOG="$DEB_ROOT/var/log/scarydb"
DEB_SERVICE="$DEB_ROOT/lib/systemd/system"
DEB_DOC="$DEB_ROOT/usr/share/doc/$PACKAGE_NAME"

mkdir -p "$DEB_BIN" "$DEB_CONFIG" "$DEB_LIB" "$DEB_LOG" "$DEB_SERVICE" "$DEB_DOC"

# Copy binary
cp target/x86_64-unknown-linux-gnu/release/scurydb "$DEB_BIN/scurydb"
chmod 755 "$DEB_BIN/scurydb"

# Copy config
cp config.json "$DEB_CONFIG/config.json"

# Create control file
cat > "$DEB_DEBIAN/control" <<EOF
Package: $PACKAGE_NAME
Version: $VERSION
Section: database
Priority: optional
Architecture: $ARCH
Maintainer: $MAINTAINER
Description: $DESCRIPTION
 ScaryDB is a high-performance, in-memory, actor-based hierarchical database 
 written in Rust. It utilizes a nested key-value store structured as 
 Database -> Bucket -> Key -> Value with automatic/explicit value types, 
 an internal ID catalog mapping, a TCP server-client architecture with a 
 thread pool/request queue concurrency model, and dual-path binary logging/
 JSON checkpoints persistence.
Homepage: https://github.com/SanjaiPS-tech/ScaryDB
Depends: libc6 (>= 2.31), libssl3, libgcc-s1
EOF

# Create postinst script
cat > "$DEB_DEBIAN/postinst" <<'EOF'
#!/bin/bash
set -e

# Create scarydb user/group
if ! getent group scarydb >/dev/null; then
    addgroup --system scarydb
fi

if ! getent passwd scarydb >/dev/null; then
    adduser --system --ingroup scarydb --home /var/lib/scarydb --shell /bin/false scarydb
fi

# Set ownership
chown -R scarydb:scarydb /var/lib/scarydb /var/log/scarydb /etc/scarydb
chmod 750 /var/lib/scarydb /var/log/scarydb
chmod 640 /etc/scarydb/config.json

# Enable systemd service
systemctl daemon-reload
systemctl enable scarydb.service

echo "ScaryDB installed. Start with: systemctl start scarydb"
EOF
chmod 755 "$DEB_DEBIAN/postinst"

# Create prerm script
cat > "$DEB_DEBIAN/prerm" <<'EOF'
#!/bin/bash
set -e

# Stop and disable service
systemctl stop scarydb.service 2>/dev/null || true
systemctl disable scarydb.service 2>/dev/null || true
EOF
chmod 755 "$DEB_DEBIAN/prerm"

# Create postrm script
cat > "$DEB_DEBIAN/postrm" <<'EOF'
#!/bin/bash
set -e

if [ "$1" = "purge" ]; then
    # Remove user/group
    deluser scarydb 2>/dev/null || true
    delgroup scarydb 2>/dev/null || true
    
    # Remove data directories
    rm -rf /var/lib/scarydb /var/log/scarydb /etc/scarydb
fi

systemctl daemon-reload
EOF
chmod 755 "$DEB_DEBIAN/postrm"

# Create systemd service
cat > "$DEB_SERVICE/scarydb.service" <<EOF
[Unit]
Description=ScaryDB Server
Documentation=https://github.com/SanjaiPS-tech/ScaryDB
After=network.target

[Service]
Type=simple
User=scarydb
Group=scarydb
ExecStart=/usr/bin/scurydb server
WorkingDirectory=/var/lib/scarydb
Restart=on-failure
RestartSec=5
LimitNOFILE=65536
LimitNPROC=4096
StandardOutput=journal
StandardError=journal
SyslogIdentifier=scarydb

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/scarydb /var/log/scarydb /etc/scarydb

[Install]
WantedBy=multi-user.target
EOF

# Create docs
cp README.md "$DEB_DOC/"
cp LICENSE "$DEB_DOC/"
cp CHANGELOG.md "$DEB_DOC/"

# Build the package
dpkg-deb --build --root-owner-group "$DEB_ROOT" "${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"

echo "Package created: ${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
echo "Install with: sudo dpkg -i ${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"