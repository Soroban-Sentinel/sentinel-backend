#!/usr/bin/env python3
"""Create a disposable schema database for SQLx compile-time query checks."""
import pathlib
import sqlite3
import sys

root = pathlib.Path(__file__).resolve().parents[1]
path = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/sentinel-check.db")
with sqlite3.connect(path) as db:
    for migration in sorted((root / "crates/sentinel-api/migrations").glob("*.sql")):
        db.executescript(migration.read_text())
print(f"DATABASE_URL=sqlite://{path.resolve()}")
