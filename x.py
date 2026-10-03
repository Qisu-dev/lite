#!/usr/bin/env python
"""lite 构建入口

用法：
    python x.py build [debug|release]
    python x.py cli [debug|release]
    python x.py check
    python x.py test
    python x.py fmt
    python x.py clean
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.resolve()
BINS = ["litec"]
OUTPUT_SUBDIR = "cli"

def is_windows() -> bool:
    return os.name == "nt"

def exe_suffix() -> str:
    return ".exe" if is_windows() else ""

def target_dir() -> Path:
    env = os.environ.get("CARGO_TARGET_DIR")
    if env:
        return Path(env)
    return ROOT / "target"

def run(args: list[str]) -> int:
    print(f"$ {' '.join(args)}")
    return subprocess.run(args, cwd=ROOT).returncode

def cmd_build(rest: list[str]) -> int:
    profile = rest[0] if rest else "debug"
    args = ["cargo", "build"]
    if profile == "release":
        args.append("--release")
    return run(args)

def cmd_cli(rest: list[str]) -> int:
    profile = rest[0] if rest else "debug"
    clean = "--clean" in rest

    if clean:
        for b in BINS:
            subprocess.run(["cargo", "clean", "-p", b], cwd=ROOT, check=True)

    args = ["cargo", "build"]
    for b in BINS:
        args += ["-p", b]
    if profile == "release":
        args.append("--release")

    rc = run(args)
    if rc != 0:
        return rc

    tdir = target_dir()
    src_dir = tdir / profile
    dst_dir = tdir / OUTPUT_SUBDIR
    dst_dir.mkdir(parents=True, exist_ok=True)

    if not src_dir.is_dir():
        print(f"错误：找不到 {src_dir}", file=sys.stderr)
        return 1

    suffix = exe_suffix()
    copied = 0
    for b in BINS:
        src = src_dir / f"{b}{suffix}"
        if not src.is_file():
            print(f"  ! 跳过 {b}")
            continue
        dst = dst_dir / f"{b}{suffix}"
        shutil.copy2(src, dst)
        print(f"  → {dst}")
        copied += 1

    if copied == 0:
        print("错误：没有拷贝任何二进制", file=sys.stderr)
        return 1

    print(f"\ndone — {dst_dir}")
    return 0

def cmd_check(_: list[str]) -> int:
    return run(["cargo", "check", "--workspace"])

def cmd_test(_: list[str]) -> int:
    return run(["cargo", "test", "--workspace"])

def cmd_fmt(_: list[str]) -> int:
    return run(["cargo", "fmt", "--all"])

def cmd_clean(_: list[str]) -> int:
    return run(["cargo", "clean"])

COMMANDS = {
    "build": cmd_build,
    "cli":   cmd_cli,
    "check": cmd_check,
    "test":  cmd_test,
    "fmt":   cmd_fmt,
    "clean": cmd_clean,
}

def main() -> int:
    args = sys.argv[1:]
    if not args or args[0] in ("-h", "--help", "help"):
        print(__doc__)
        return 0

    cmd, *rest = args
    handler = COMMANDS.get(cmd)
    if handler is None:
        print(f"未知子命令：{cmd}", file=sys.stderr)
        print(__doc__)
        return 2

    return handler(rest)

if __name__ == "__main__":
    sys.exit(main())