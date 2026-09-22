#!/usr/bin/env python3
"""一致性测试框架 —— C++(oracle) 与 Rust(移植) 行为等价比对。

工作方式:
  1. 取 C++ 参考二进制(emf_common_tests)运行, 得到 oracle.json(每个 C++ 测试 pass/fail)。
  2. 对 cases.tsv 中已映射的测试, 用 `cargo test -p emf-common <name> -- --exact` 单独跑 Rust 侧。
  3. 汇总结论:
       PASS         oracle 通过 且 Rust 对应测试通过   -> 行为等价
       PENDING      尚未映射 或 Rust 对应测试缺失       -> 等价待实现(可接受, 不使构建失败)
       REGRESSION   oracle 通过 但 Rust 对应测试失败   -> 必须修复

用法:
  python3 tools/conformance/compare.py [--oracle build/oracle.json] [--pkg emf-common]
退出码: 0(无 REGRESSION); 1(出现 REGRESSION)。
"""
import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
BUILD = os.path.join(HERE, "build")

CARGO = os.environ.get("CARGO", "cargo")


def parse_tsv(path):
    rows = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) == 3 and parts[0] == "group":
            continue  # header
        if len(parts) == 3:
            rows.append(tuple(parts))
    return rows


def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd or ROOT, capture_output=True, text=True)


def oracle(path):
    return json.load(open(path, encoding="utf-8"))


def rust_names(pkg):
    r = run([CARGO, "test", "-p", pkg, "--", "--list"])
    names = set()
    for line in r.stdout.splitlines():
        if line.endswith(": test"):
            names.add(line[: -len(": test")])
    return names


def rust_pass_any(pkg, pattern):
    r = run([CARGO, "test", "-p", pkg, pattern, "--", "--exact"])
    return "test result: ok." in r.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--oracle", default=os.path.join(BUILD, "oracle.json"))
    ap.add_argument("--pkg", default="emf-common")
    ap.add_argument("--cases", default=os.path.join(HERE, "cases.tsv"))
    args = ap.parse_args()

    if not os.path.exists(args.oracle):
        sys.exit(f"oracle 缺失: {args.oracle}\n先运行 tools/conformance/build_oracle.sh")

    o = oracle(args.oracle)
    rows = parse_tsv(args.cases)
    names = rust_names(args.pkg)

    mapped = [(g, c, rt) for g, c, rt in rows if rt]
    by_cpp = {(c): (g, rt) for g, c, rt in mapped}

    pending = 0
    passed = 0
    regress = []

    for cpp, status in sorted(o.items()):
        if cpp in by_cpp:
            g, rt = by_cpp[cpp]
            if status != "pass" or rt not in names:
                pending += 1
                verdict = "PENDING"
                note = "oracle未通过" if status != "pass" else f"Rust测试缺失: {rt}"
            else:
                if rust_pass_any(args.pkg, rt):
                    passed += 1
                    verdict = "PASS"
                    note = rt
                else:
                    regress.append((g, cpp, rt))
                    verdict = "REGRESSION"
                    note = rt
            print(f"[{verdict:10}] {g:<18} {cpp:<42} {note}")
        else:
            pending += 1

    # 统计尚有 C++ 测试完全未映射的分组, 便于逐步补齐
    unmapped = sorted({c for c in o if c not in by_cpp})
    print("\n==== 汇总 ====")
    print(f"oracle 测试总数: {len(o)}  通过: {sum(1 for v in o.values() if v=='pass')}")
    print(f"已映射: {len(by_cpp)}  等价(PASS): {passed}  待实现(PENDING): {pending}")
    print(f"未映射 C++ 测试数: {len(unmapped)}")
    if unmapped:
        print("未映射示例(首 8 个):", ", ".join(unmapped[:8]))
    if regress:
        print("REGRESSION(必须修复):")
        for g, c, rt in regress:
            print(f"  {g} :: {c}  <- Rust {rt} 失败")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())