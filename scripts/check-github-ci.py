#!/usr/bin/env python3
"""Read public Actions status for a backend commit without account credentials."""
import argparse
import json
import time
import urllib.request

BASE = "https://api.github.com/repos/Soroban-Sentinel/sentinel-backend"


def get(path):
    request = urllib.request.Request(BASE + path, headers={"Accept": "application/vnd.github+json", "User-Agent": "sentinel-ci-verification"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("commit")
    parser.add_argument("--wait", action="store_true")
    args = parser.parse_args()
    if len(args.commit) != 40 or any(c not in "0123456789abcdef" for c in args.commit):
        parser.error("expected a full lowercase commit SHA")
    deadline = time.monotonic() + (1200 if args.wait else 0)
    previous = None
    while True:
        runs = get(f"/actions/runs?head_sha={args.commit}&per_page=5")["workflow_runs"]
        run = next((r for r in runs if r["name"] == "Backend CI"), None)
        if run:
            jobs = get(f"/actions/runs/{run['id']}/jobs")["jobs"]
            result = {"commit": args.commit, "url": run["html_url"], "status": run["status"],
                      "conclusion": run["conclusion"], "jobs": [{"name": j["name"], "status": j["status"],
                      "conclusion": j["conclusion"], "steps": [{"name": s["name"], "status": s["status"],
                      "conclusion": s["conclusion"]} for s in j.get("steps", [])]} for j in jobs]}
            serialized = json.dumps(result, sort_keys=True)
            if serialized != previous:
                print(json.dumps(result, indent=2), flush=True)
                previous = serialized
            if run["status"] == "completed":
                if run["conclusion"] != "success":
                    for job in jobs:
                        if job["conclusion"] != "failure":
                            continue
                        try:
                            request = urllib.request.Request(BASE + f"/actions/jobs/{job['id']}/logs", headers={"User-Agent": "sentinel-ci-verification"})
                            with urllib.request.urlopen(request, timeout=30) as response:
                                print(response.read().decode(errors="replace")[-18000:], flush=True)
                        except Exception as error:
                            print(f"Could not read public job log: {error}", flush=True)
                            annotations_path = job["check_run_url"].removeprefix(BASE) + "/annotations"
                            print(json.dumps(get(annotations_path), indent=2), flush=True)
                return 0 if run["conclusion"] == "success" else 1
        if time.monotonic() >= deadline:
            return 2
        time.sleep(10)


if __name__ == "__main__":
    raise SystemExit(main())
