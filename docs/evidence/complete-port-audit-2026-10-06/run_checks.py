"""Read-only-source audit gates. Does not launch the desktop app or OS tests."""
import argparse
import datetime
import json
from pathlib import Path
import re
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
APP = ROOT / "apps/desktop"
CARGO = Path("C:/Users/Colby/.cargo/bin/cargo.exe")
FLUTTER = Path("C:/Users/Colby/toolchains/flutter/bin/flutter.bat")
DART = Path("C:/Users/Colby/toolchains/flutter/bin/dart.bat")


def run(name, command, cwd, timeout=1800):
    log = HERE / "checks" / (name + ".log")
    log.parent.mkdir(parents=True, exist_ok=True)
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    tic = time.monotonic()
    with log.open("w", encoding="utf-8") as out:
        try:
            result = subprocess.run(command, cwd=cwd, stdout=out,
                                    stderr=subprocess.STDOUT, timeout=timeout)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = "timeout"
    record = {"name": name, "command": [str(x) for x in command],
              "cwd": str(cwd), "started_at": started,
              "seconds": round(time.monotonic() - tic, 3),
              "exit_code": code, "log": str(log.relative_to(ROOT))}
    print(json.dumps(record, ensure_ascii=False), flush=True)
    return record


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=["rust", "flutter", "flutter-tests", "flutter-batch", "build"])
    args = parser.parse_args()
    records = []
    if args.kind == "rust":
        for name, extra in [("cargo-fmt", ["fmt", "--all", "--", "--check"]),
                            ("cargo-clippy", ["clippy", "--workspace", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]),
                            ("cargo-test", ["test", "--workspace", "--locked", "--offline"])]:
            records.append(run(name, [str(CARGO), *extra], ROOT))
    if args.kind == "flutter":
        records.append(run("dart-format", [str(DART), "format", "--output=none", "--set-exit-if-changed", "lib", "test"], APP))
        records.append(run("flutter-analyze", [str(FLUTTER), "analyze", "--no-pub"], APP))
    if args.kind == "flutter-tests":
        for file in sorted((APP / "test").rglob("*_test.dart")):
            rel = file.relative_to(APP).as_posix()
            name = "flutter-" + re.sub(r"[^a-zA-Z0-9_.-]", "_", rel)
            record = run(name, [str(FLUTTER), "test", "--no-pub", rel, "--reporter", "expanded"], APP, 180)
            records.append(record)
            # Preserve the first failure, even when retrying a confirmed tester crash.
            log_text = (ROOT / record["log"]).read_text(encoding="utf-8", errors="replace")
            if record["exit_code"] != 0 and "Shell::Create" in log_text:
                records.append(run(name + "-engine-retry", [str(FLUTTER), "test", "--no-pub", rel, "--reporter", "expanded"], APP, 180))
    if args.kind == "flutter-batch":
        records.append(run("flutter-batch", [str(FLUTTER), "test", "--no-pub", "--concurrency=1", "--reporter", "expanded"], APP, 1800))
    if args.kind == "build":
        records.append(run("cargo-release", [str(CARGO), "build", "--workspace", "--release", "--locked", "--offline"], ROOT))
        records.append(run("flutter-release", [str(FLUTTER), "build", "windows", "--release", "--no-pub"], APP))
    (HERE / "checks" / (args.kind + "-results.json")).write_text(
        json.dumps(records, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
