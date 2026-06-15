# ScaryDB 🎃

A high-performance, in-memory, actor-based hierarchical database written in Rust. It utilizes a nested key-value store structured as `Database -> Bucket -> Key -> Value` with automatic/explicit value types, an internal ID catalog mapping, a TCP server-client architecture with a thread pool/request queue concurrency model, and dual-path binary logging/JSON checkpoints persistence.

---

## 🚀 Getting Started (CLI Modes)

ScaryDB compiles into a multi-purpose executable that supports three running modes. Run these using Cargo:

### 1. Start the Database Server
Starts the TCP server on the host and port defined in `config.json`.
```bash
cargo run -- server
```
>*Note: If no mode argument is supplied, ScaryDB defaults to running as a server.*

### 2. Launch the Interactive REPL Client
Launches the command-line client to connect and execute queries against a running server.
```bash
cargo run -- client
```

### 3. Read the Binary WAL Log
Decodes and translates a binary transaction log (`operations.log`) into a human-readable text stream of transaction actions.
```bash
cargo run -- log-read <path_to_operations.log>
# Example:
cargo run -- log-read ./data/operations.log
```

---

## 🛠️ Database Query Syntax (Interactive REPL)

Once connected via the client, you can execute case-insensitive SQL-like queries. Multiple operations can be chained together using the `/` separator, and statements optionally terminate with a semicolon `;`.

### 1. Database Definition Commands (DDC)
Used to structure database context and bucket namespaces:
*   `CREATE DB <db_name>;` - Create a new database.
*   `DROP DB <db_name>;` - Delete a database.
*   `USE <db_name>;` - Switch the active connection to a specific database context.
*   `CREATE BUCKET <bucket_name>;` - Create a bucket under the current active database.
*   `DROP BUCKET <bucket_name>;` - Drop a bucket and all its keys under the current active database.
*   `LIST DBS;` (or `LIST DATABASES;`) - List all databases.
*   `LIST BUCKETS;` (or `LIST BUCK;`) - List all buckets in the active database.

### 2. Data Manipulation Commands (DMC)
Used to mutate key-value pairs:
*   `SET <bucket> <key> [TYPE_TAG] <value> [ / <key2> [TYPE_TAG] <value2> ... ];`
    *   Set one or more key-value pairs in a bucket.
    *   **Type Tags (Optional):** `[STRING]`, `[INT]`, `[FLOAT]`, or `[BOOL]`. If omitted, type is automatically detected (e.g. `42` becomes `Int`, `true` becomes `Bool`, `"hello"` becomes `String`).
    *   *Example:* `SET users user1 "Alice" / user2 [INT] 42 / user3 [BOOL] true;`
*   `DEL <bucket> <key> [ / <key2> ... ];` - Delete one or more keys from a bucket.

### 3. Data Retrieval Commands (DRC)
Used to query key-value pairs:
*   `GET <bucket> <key> [ / <key2> ... ];` - Retrieve the value of one or more keys. Missing keys return `(nil)`.
*   `EXISTS <bucket> <key> [ / <key2> ... ];` - Check if one or more keys exist (returns `true` or `false`).
*   `LIST <bucket>;` - List all key names inside a bucket.
*   `COUNT <bucket>;` - Get the count of keys inside a bucket.

### 4. System Control Commands (SCC)
Used for checking server status, latency, and versions:
*   `PING` (or `BOINK`) - Test connectivity. Returns `BOINK! 🐷`.
*   `INFO` - Returns server startup timestamps, data directory paths, thread worker configurations, and memory limits.
*   `STATS` - Returns count metrics for databases, buckets, and keys.
*   `VERSION` - Returns current version of ScaryDB.
*   `HELP` (or `MAN`) - Displays command syntax instructions inside the REPL.
*   `EXIT` (or `QUIT`) - Disconnects from the server and exits the REPL.

### 5. Configuration Control Commands (CCC)
Used to view and update server configurations at runtime:
*   `LIST CONFIG;` - Lists all server configurations.
*   `GET CONFIG <property>;` - Get the value of a configuration property.
*   `SET CONFIG <property> <value>;` - Set a configuration property (saves updates to `config.json` automatically).
    *   *Example:* `SET CONFIG storage.checkpoint_interval_ops 100`

---

## ⚙️ Configuration Properties (`config.json`)

The following settings are managed in `config.json`:
*   `server.workers`: Number of thread pool worker threads (default is `1` for lock-free storage execution).
*   `storage.data_dir`: The directory path where database state snapshots and write-ahead logs are persisted.
*   `storage.checkpoint_interval_ops`: The number of mutation operations allowed before a JSON snapshot checkpoint is written and the WAL log is truncated.
*   `memory.max_memory_kb`: Maximum memory threshold for warning limits.
*   `network.host` / `network.port`: Connection binding settings.

---

# ⚠️ Some known errors we have encountered.

## ⚠️ Troubleshooting: OS Error 32 (File in use)

If you see an error like this when building:
> `error: failed to remove ... target\debug\deps\scarydb.exe: The process cannot access the file because it is being used by another process. (os error 32)`

This happens because the ScaryDB Server or REPL Client is running in the background. Cargo cannot overwrite the binary while it is active. To resolve:

### 1. Shut down running ScaryDB instances
*   **From the running terminal:** Press `Ctrl + C` to send a graceful termination signal.
*   **From a separate terminal (Force terminate):**
    *   **Windows (PowerShell):**
        ```powershell
        Stop-Process -Name scarydb -Force
        ```
    *   **Windows (CMD):**
        ```cmd
        taskkill /F /IM scarydb.exe
        ```
    *   **Linux / macOS:**
        ```bash
        killall scarydb
        # or
        pkill scarydb
        ```

### 🔒 Safety Precautions & Data Integrity
*   **Final Checkpoints:** ScaryDB executes a graceful final checkpoint (`catalog.db` and database JSON state serialization) on clean exit to save everything to disk.
*   **WAL Persistence:** If you are forced to use a force-kill command (e.g. `Stop-Process` or `taskkill /F`), the final checkpoint will be skipped. However, ScaryDB's Write-Ahead Log (`operations.log`) commits mutations immediately in binary format. On the next start, the engine will replay the WAL transactions to restore your state.
*   **Best Practice:** Always try sending a standard interrupt signal (`Ctrl + C` or normal termination) before executing a force-kill, to ensure the JSON checkpoint files and catalogs are perfectly synchronized.

### 2. Run `cargo build` or `cargo run` again
Once all instances are stopped, the file lock is released, and Cargo will compile successfully.