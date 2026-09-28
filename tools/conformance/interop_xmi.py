#!/usr/bin/env python3
"""双向 XMI 互操作验证：证明 Rust 与 C++ 两个移植版本能互相读写对方的文件。

与 compare.py（对同一批 fixture 比对"行为一致"）不同，这里做的是真正的跨进程
文件交接：

  A. Rust 写文件  -> C++ 读回并校验
  B. C++  写文件  -> Rust 读回并校验
  C. Rust 读 C++ 的文件后写回 -> C++ 再读回并校验
  D. C++  读 Rust 的文件后写回 -> Rust 再读回并校验

两侧 harness（tools/conformance/interop_xmi_main.cpp 与
crates/emf-ecore-codegen/examples/interop_xmi.rs）构建同一份 library 模型与同一
份实例数据，因此任一侧产出的文件都必须通过另一侧的 check。

用法:
  python3 tools/conformance/interop_xmi.py \
      --cpp tools/conformance/build/interop_xmi \
      --rust target/debug/examples/interop_xmi
退出码: 0(全部通过); 1(存在失败)。
"""
import argparse
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def resolve(path):
    return path if os.path.isabs(path) else os.path.join(ROOT, path)


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cpp", required=True, help="C++ interop_xmi harness binary")
    ap.add_argument("--rust", required=True, help="Rust interop_xmi example binary")
    args = ap.parse_args()

    cpp = resolve(args.cpp)
    rust = resolve(args.rust)
    for label, path in (("C++", cpp), ("Rust", rust)):
        if not os.path.exists(path):
            sys.exit(f"{label} harness missing: {path}")

    failures = []

    def step(name, cmd, expect):
        r = run(cmd)
        ok = r.returncode == 0 and expect in r.stdout
        tag = "PASS" if ok else "FAIL"
        print(f"[{tag}] {name}")
        if not ok:
            print(f"       cmd: {' '.join(cmd)}")
            print(f"       rc={r.returncode}")
            if r.stdout.strip():
                print(f"       stdout: {r.stdout.strip()}")
            if r.stderr.strip():
                print(f"       stderr: {r.stderr.strip()}")
            failures.append(name)

    with tempfile.TemporaryDirectory(prefix="xmi-interop-") as tmp:
        rust_out = os.path.join(tmp, "rust_out.xmi")
        cpp_out = os.path.join(tmp, "cpp_out.xmi")
        rust_echo = os.path.join(tmp, "rust_echo.xmi")
        cpp_echo = os.path.join(tmp, "cpp_echo.xmi")

        # A. Rust -> C++
        step("A. Rust emit", [rust, "emit", rust_out], "EMIT-OK")
        step("A. C++ check Rust file", [cpp, "check", rust_out], "CHECK-OK")

        # B. C++ -> Rust
        step("B. C++ emit", [cpp, "emit", cpp_out], "EMIT-OK")
        step("B. Rust check C++ file", [rust, "check", cpp_out], "CHECK-OK")

        # C. Rust 读 C++ 文件后写回 -> C++ 再读
        step("C. Rust roundtrip C++ file", [rust, "roundtrip", cpp_out, rust_echo], "ROUNDTRIP-OK")
        step("C. C++ check Rust re-emit", [cpp, "check", rust_echo], "CHECK-OK")

        # D. C++ 读 Rust 文件后写回 -> Rust 再读
        step("D. C++ roundtrip Rust file", [cpp, "roundtrip", rust_out, cpp_echo], "ROUNDTRIP-OK")
        step("D. Rust check C++ re-emit", [rust, "check", cpp_echo], "CHECK-OK")

    if failures:
        print(f"\nxmi interop: {len(failures)} step(s) failed")
        return 1
    print("\nxmi interop: all 8 steps passed (Rust <-> C++ bidirectional)")
    return 0


if __name__ == "__main__":
    sys.exit(main())