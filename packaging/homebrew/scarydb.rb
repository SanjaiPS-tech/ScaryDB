class Scarydb < Formula
  desc "High-performance, in-memory, actor-based hierarchical database"
  homepage "https://github.com/SanjaiPS-tech/ScaryDB"
  version "0.1.0"
  license "MIT OR Apache-2.0"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v#{version}/scarydb-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_X86_64"
    else
      url "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v#{version}/scarydb-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_ARM64"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v#{version}/scarydb-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_X86_64_LINUX"
    else
      url "https://github.com/SanjaiPS-tech/ScaryDB/releases/download/v#{version}/scarydb-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_ARM64_LINUX"
    end
  end

  def install
    bin.install "scarydb"
    bin.install "scarydb.sh"
    (etc/"scarydb").install "config.json"
    (var/"lib/scarydb").mkpath
    (var/"log/scarydb").mkpath
  end

  def post_install
    # Create default config if not exists
    unless (etc/"scarydb/config.json").exist?
      (etc/"scarydb").install "config.json"
    end
  end

  service do
    run [opt_bin/"scarydb", "server"]
    keep_alive true
    working_dir var/"lib/scarydb"
    log_path var/"log/scarydb/server.log"
    error_log_path var/"log/scarydb/error.log"
  end

  test do
    system "#{bin}/scarydb", "--version"
    assert_match "ScaryDB v#{version}", shell_output("#{bin}/scarydb --version")
  end
end