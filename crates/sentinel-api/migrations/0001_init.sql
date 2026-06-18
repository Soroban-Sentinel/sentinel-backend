-- Sentinel DB schema

CREATE TABLE IF NOT EXISTS runs (
    id           TEXT PRIMARY KEY,
    repo         TEXT NOT NULL,
    commit_sha   TEXT NOT NULL,
    pr_number    INTEGER,
    status       TEXT NOT NULL DEFAULT 'queued',
    created_at   TEXT NOT NULL,
    completed_at TEXT,
    coverage_pct REAL,
    finding_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS findings (
    id            TEXT PRIMARY KEY,
    run_id        TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    contract_name TEXT NOT NULL,
    kind          TEXT NOT NULL,
    severity      TEXT NOT NULL,
    description   TEXT NOT NULL,
    reproducer    TEXT,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS coverage_snapshots (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id        TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    contract_name TEXT NOT NULL,
    coverage_pct  REAL NOT NULL,
    unique_edges  INTEGER NOT NULL DEFAULT 0,
    snapshot_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_findings_run_id ON findings(run_id);
CREATE INDEX IF NOT EXISTS idx_coverage_contract ON coverage_snapshots(contract_name);
