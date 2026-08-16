# Database Schema

The canonical SQLite schema lives in [`src-tauri/src/db.rs`](../src-tauri/src/db.rs) (function `Database::migrate`). This document mirrors it and must be updated in the same commit as any schema change.

## File location

| OS    | Path                                                     |
|-------|----------------------------------------------------------|
| macOS | `~/Library/Application Support/portsage/portsage.db`     |
| Linux | `$XDG_DATA_HOME/portsage/portsage.db` (default `~/.local/share/portsage/portsage.db`) |

Path resolution is centralised in [`src-tauri/src/paths.rs`](../src-tauri/src/paths.rs).

## Tables

### `projects`

```sql
CREATE TABLE projects (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    path        TEXT,
    range_start INTEGER NOT NULL,
    range_end   INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    archived_at TEXT                                    -- NULL = live
);
```

A project owns a contiguous port range `[range_start, range_end]`. Ranges never overlap - allocation is performed under a single mutex lock (see `Database::create_project` + `compute_next_range`) to defeat the read-modify-write race. The regression test `db.rs::concurrent_create_project_produces_no_overlapping_ranges` covers this.

`name` is unique; `path` is optional and points to a project directory on disk (used by `find_project_by_path` and "Open in Finder/Terminal").

`archived_at` shelves a project: it keeps its name, its range and its ports, and is subject to every constraint as before, it just drops out of the default listing. This is not a soft delete - the row is fully live - so a query that forgets to filter it shows one row too many rather than corrupting anything. Set by `prune` / `portsage archive`, cleared by `portsage unarchive` and automatically by `Database::unarchive_projects_with_active_ports` when one of the project's ports starts listening again.

New columns are added by `db.rs::add_column_if_missing`, because the `CREATE TABLE IF NOT EXISTS` in `migrate` never touches a table that already exists.

### `ports`

```sql
CREATE TABLE ports (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    service    TEXT NOT NULL,
    port       INTEGER NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Each row is one `(project, service, port)` triple. `port` is globally unique. The `project_id` FK is informational - cleanup on project deletion is performed in code (`Database::delete_project` deletes rows from `ports` first, then from `projects`). Both deletion paths archive what they remove in `trash` first, inside the same transaction.

### `config`

```sql
CREATE TABLE config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO config (key, value) VALUES ('base_port', '4000');
INSERT OR IGNORE INTO config (key, value) VALUES ('range_size', '10');
```

Free-form key/value store. Only two keys are accepted today (`base_port`, `range_size`). Values are TEXT in SQLite and converted at the boundary - the wire type `ConfigSnapshot` keeps them as strings on purpose to avoid silent precision loss.

### `remote_backends` (multi-host, Phase 2)

```sql
CREATE TABLE remote_backends (
    id                   INTEGER PRIMARY KEY,
    name                 TEXT NOT NULL UNIQUE,
    ssh_alias            TEXT NOT NULL,
    remote_socket_path   TEXT NOT NULL,
    local_socket_path    TEXT NOT NULL,
    auto_forward_enabled INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Catalogue of remote Portsage servers the Mac UI knows about. Meaningful only on the Mac - on a Linux server this table stays empty (the server is itself a backend, not a consumer of remotes).

- `ssh_alias` resolves through the user's `~/.ssh/config`. Portsage does not duplicate ssh's host/user/port/identity logic.
- `remote_socket_path` is the absolute path of the socket on the remote box, e.g. `/run/portsage/portsage.sock`.
- `local_socket_path` is the local side of the `ssh -L unix:<local>:<remote>` forward; lives under `paths::state_dir()/<alias>.sock`.
- `auto_forward_enabled` (0/1) gates the Phase 3 auto-forward feature for this backend.

The row type re-exports `portsage_client::RemoteBackend` so the wire shape and the on-disk shape cannot drift.

### `forward_exclusions` (multi-host, Phase 3)

```sql
CREATE TABLE forward_exclusions (
    id         INTEGER PRIMARY KEY,
    backend_id INTEGER NOT NULL REFERENCES remote_backends(id),
    port       INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(backend_id, port)
);
```

Per-backend blocklist of ports the user does not want auto-forwarded (e.g. a port already in use locally by an unrelated process). Cascade on backend deletion is performed in code (`Database::delete_remote_backend` deletes exclusions first); the FK stays informational so error reporting matches the rest of the CRUD path. Regression test: `db.rs::delete_remote_backend_cascades_forward_exclusions`.

### `trash`

```sql
CREATE TABLE trash (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL,              -- 'project' | 'port'
    label           TEXT NOT NULL,              -- 'liber' | 'mcpbelt / postgres'
    detail          TEXT NOT NULL,              -- 'range 4060-4069, 6 ports' | 'port 4332'
    payload         TEXT NOT NULL,              -- JSON snapshot, restored verbatim
    payload_version INTEGER NOT NULL DEFAULT 1,
    deleted_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Archive of deleted projects and ports, so a mistaken `release` or `remove` can be undone. `Database::delete_project` and `Database::remove_port` write the snapshot and delete the live rows in one transaction, so the archive cannot be skipped by a caller or lost to a crash between the two writes.

Why an archive rather than a `deleted_at` column on `projects` / `ports`:

- `projects.name` and `ports.port` carry inline UNIQUE constraints. SQLite cannot drop the implicit indexes those create, so partial unique indexes (`WHERE deleted_at IS NULL`) would mean rebuilding both tables.
- Every query in `db.rs` would need a `deleted_at IS NULL` filter, and a forgotten one is a silent bug.
- A project plus its ports is a closed aggregate: no other table references either, and no query wants to see deleted rows.
- Soft delete would not even simplify the restore. With a partial unique index a deleted port and a live one can coexist, so the restore would still have to resolve the conflict by hand.

`payload` shapes (`payload_version = 1`), serialized from `db.rs::TrashPayload`:

```json
{"kind":"project","name":"liber","path":"/Users/x/liber","range_start":4060,
 "range_end":4069,"created_at":"...","ports":[{"service":"vite","port":4060,"created_at":"..."}]}

{"kind":"port","project_name":"liber","service":"postgres","port":4062,"created_at":"..."}
```

A restore rejects a name collision, an overlapping range, or a missing parent project; ports taken by a live project in the meantime are skipped and reported in `RestoreOutcome::skipped_ports`. Retention is `TRASH_RETENTION_DAYS` (30), enforced by `purge_trash_older_than` called from `Database::new` - on open, not from a command, so no caller can forget it.

## Invariants

- **No overlapping ranges.** `compute_next_range` always uses `MAX(range_end) + 1` (or `base_port` for the empty case), and runs under the same lock as the insert.
- **Globally unique port numbers** across projects (the UNIQUE constraint on `ports.port` is the safety net; the application layer also validates that the port falls inside the project's range).
- **No hard delete cascades from SQL** - all cleanup happens in code so error messages stay consistent.
- **Hard delete plus an archive, not soft delete.** Projects and ports are deleted for real; the snapshot in `trash` is what makes the deletion reversible for 30 days. The reasoning is under the `trash` table above.
- **Archiving never frees a range.** An archived project still owns its numbers, and `compute_next_range` still counts it. Shelving is about the list, not about reclaiming ports (regression test: `db.rs::archiving_does_not_free_the_range_for_the_next_project`).
- **Ranges are never recycled.** A deleted project's range stays free until `MAX(range_end)` moves past it, which is what makes restoring a project with its original range safe: the `.env` and compose files that reference those ports keep working.

## Migrations

There is no migration framework. `Database::migrate` runs `CREATE TABLE IF NOT EXISTS` on every startup, so adding a new column requires either a fresh DB or a manual `ALTER TABLE` plus a guarded `IF NOT EXISTS` in the migration string. When that day comes, switch to a numbered migration helper rather than stacking `ALTER` statements in `migrate`.
