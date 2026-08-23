Name:           scarydb
Version:        0.1.0
Release:        1%{?dist}
Summary:        High-performance, in-memory, actor-based hierarchical database

License:        MIT OR Apache-2.0
URL:            https://github.com/SanjaiPS-tech/ScaryDB
Source0:        https://github.com/SanjaiPS-tech/ScaryDB/archive/v%{version}/ScaryDB-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  openssl-devel
Requires:       libcrypto
Requires:       libssl
Requires:       systemd

%description
ScaryDB is a high-performance, in-memory, actor-based hierarchical database written in Rust.
It utilizes a nested key-value store structured as Database -> Bucket -> Key -> Value with
automatic/explicit value types, an internal ID catalog mapping, a TCP server-client architecture
with a thread pool/request queue concurrency model, and dual-path binary logging/JSON
checkpoints persistence.

%prep
%autosetup -p1

%build
export CARGO_HOME=%{_builddir}/.cargo
export RUSTUP_HOME=%{_builddir}/.rustup
cargo build --release

%install
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_sysconfdir}/scarydb
mkdir -p %{buildroot}%{_localstatedir}/lib/scarydb
mkdir -p %{buildroot}%{_localstatedir}/log/scarydb
mkdir -p %{buildroot}%{_unitdir}
mkdir -p %{buildroot}%{_docdir}/scarydb

install -m 755 target/release/scurydb %{buildroot}%{_bindir}/scurydb
install -m 644 config.json %{buildroot}%{_sysconfdir}/scarydb/config.json
install -m 644 README.md %{buildroot}%{_docdir}/scarydb/
install -m 644 LICENSE %{buildroot}%{_docdir}/scarydb/
install -m 644 CHANGELOG.md %{buildroot}%{_docdir}/scarydb/

# Systemd service
cat > %{buildroot}%{_unitdir}/scarydb.service <<EOF
[Unit]
Description=ScaryDB Server
Documentation=https://github.com/SanjaiPS-tech/ScaryDB
After=network.target

[Service]
Type=simple
User=scarydb
Group=scarydb
ExecStart=%{_bindir}/scurydb server
WorkingDirectory=%{_localstatedir}/lib/scarydb
Restart=on-failure
RestartSec=5
LimitNOFILE=65536
LimitNPROC=4096
StandardOutput=journal
StandardError=journal
SyslogIdentifier=scarydb

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=%{_localstatedir}/lib/scarydb %{_localstatedir}/log/scarydb %{_sysconfdir}/scarydb

[Install]
WantedBy=multi-user.target
EOF

%pre
getent group scarydb >/dev/null || groupadd -r scarydb
getent passwd scarydb >/dev/null || useradd -r -g scarydb -d %{_localstatedir}/lib/scarydb -s /sbin/nologin -c "ScaryDB Database Server" scarydb

%post
systemctl daemon-reload
systemctl enable scarydb.service >/dev/null || :

%preun
%systemd_preun scarydb.service

%postun
%systemd_postun scarydb.service
systemctl daemon-reload

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/scurydb
%config(noreplace) %{_sysconfdir}/scarydb/config.json
%dir %{_localstatedir}/lib/scarydb
%dir %{_localstatedir}/log/scarydb
%attr(0750,scarydb,scarydb) %{_localstatedir}/lib/scarydb
%attr(0750,scarydb,scarydb) %{_localstatedir}/log/scarydb
%attr(0640,scarydb,scarydb) %{_sysconfdir}/scarydb/config.json
%{_unitdir}/scarydb.service

%changelog
* Mon Aug 19 2026 SanjaiPS-tech <maintainers@scarydb.io> - 0.1.0-1
- Initial RPM release