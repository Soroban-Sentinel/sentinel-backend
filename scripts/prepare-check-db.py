#!/usr/bin/env python3
"""Create a disposable schema database for SQLx compile-time query checks."""
import pathlib
import sqlite3
import sys

root = pathlib.Path(__file__).resolve().parents[1]
path = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/sentinel-check.db")
with sqlite3.connect(path) as db:
    db.execute("CREATE TABLE IF NOT EXISTS check_migrations (name TEXT PRIMARY KEY)")
    for migration in sorted((root / "crates/sentinel-api/migrations").glob("*.sql")):
        if not db.execute("SELECT 1 FROM check_migrations WHERE name = ?", (migration.name,)).fetchone():
            db.executescript(migration.read_text())
            db.execute("INSERT INTO check_migrations VALUES (?)", (migration.name,))
print(f"DATABASE_URL=sqlite://{path.resolve()}")
