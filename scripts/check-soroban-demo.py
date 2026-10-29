#!/usr/bin/env python3
"""Exercise real Soroban regressions through both the CLI and authenticated API.

Run after building the binaries and committing the fixture: analysis intentionally
uses Git HEAD, not uncommitted files. A broken detector or a broken fix fails CI.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--output", default="/tmp/sentinel-soroban-evidence.json")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(root / "examples/soroban-token/target")
    environment["CARGO_BUILD_JOBS"] = "2"
    environment["CARGO_NET_OFFLINE"] = "true"
    cli = root / "target/debug/sentinel"
    api = root / "target/debug/sentinel-api"
    evidence = {"cli": {}, "api": {}}
    for variant, expected_code, expected_status in [("vulnerable", 1, "failed"), ("fixed", 0, "passed")]:
        result = subprocess.run([str(cli), str(root / f"examples/{variant}.sentinel.toml"), str(root)],
                                env=environment, capture_output=True, text=True, timeout=360)
        assert result.returncode == expected_code, (variant, result.returncode, result.stderr)
        report = json.loads(result.stdout)
        assert report["status"] == expected_status, report
        assert report["results"][0]["checks_executed"] == 3, report
        assert report["results"][0]["coverage_pct"] is None
        assert len(report["results"][0]["findings"]) == (1 if variant == "vulnerable" else 0), report
        evidence["cli"][variant] = report
        print(f"CLI {variant}: {expected_status}; three real Soroban checks executed", flush=True)

    with tempfile.TemporaryDirectory(prefix="sentinel-api-demo-") as directory:
        token = secrets.token_hex(32)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        environment.update(SENTINEL_API_TOKEN=token, SENTINEL_WORKSPACE_ROOT=str(root),
                           SENTINEL_BIND_ADDR=f"127.0.0.1:{port}", DATABASE_URL=f"sqlite://{directory}/demo.db")
        log_path = Path(directory) / "api.log"
        with log_path.open("w+") as log:
            process = subprocess.Popen([str(api)], env=environment, stdout=log, stderr=log)
            try:
                def request(path, body=None, authenticated=True):
                    headers = {"Content-Type": "application/json"}
                    if authenticated:
                        headers["Authorization"] = f"Bearer {token}"
                    data = None if body is None else json.dumps(body).encode()
                    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", data=data, headers=headers)
                    try:
                        with urllib.request.urlopen(req, timeout=15) as response:
                            return response.status, json.load(response)
                    except urllib.error.HTTPError as error:
                        return error.code, json.load(error)

                for _ in range(200):
                    if process.poll() is not None:
                        log.seek(0)
                        raise AssertionError(log.read())
                    try:
                        assert request("/runs", authenticated=False)[0] == 401
                        break
                    except (ConnectionError, urllib.error.URLError):
                        time.sleep(0.05)
                else:
                    raise AssertionError("API did not become ready")
                assert request("/runs")[0] == 200
                example = evidence["cli"]["fixed"]
                payload = {"repo": example["repository"], "commit_sha": example["source_commit"], "config_b64": "invalid"}
                assert request("/runs", payload)[0] == 400
                payload["config_b64"] = base64.b64encode((root / "examples/fixed.sentinel.toml").read_bytes()).decode()
                spoof = dict(payload, commit_sha="0" * 40)
                assert request("/runs", spoof)[0] == 400
                for variant, expected in [("vulnerable", "failed"), ("fixed", "passed")]:
                    payload["config_b64"] = base64.b64encode((root / f"examples/{variant}.sentinel.toml").read_bytes()).decode()
                    code, created = request("/runs", payload)
                    assert code == 201, created
                    for _ in range(3000):
                        code, report = request(f"/runs/{created['id']}")
                        assert code == 200
                        if report["status"] not in ("queued", "running"):
                            break
                        time.sleep(0.1)
                    assert report["status"] == expected, report
                    assert report["analysis_report"]["source_commit"] == example["source_commit"]
                    assert report["analysis_report"]["results"][0]["checks_executed"] == 3
                    assert report["coverage_pct"] is None
                    code, findings = request(f"/runs/{created['id']}/findings")
                    assert code == 200
                    assert len(findings) == (1 if variant == "vulnerable" else 0), findings
                    assert report["finding_count"] == len(findings)
                    evidence["api"][variant] = report
                    print(f"API {variant}: {expected}; {len(findings)} persisted finding(s)", flush=True)
            finally:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
    Path(args.output).write_text(json.dumps(evidence, indent=2) + "\n")
    print(f"Evidence saved to {args.output}")


if __name__ == "__main__":
    main()
