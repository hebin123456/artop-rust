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
    """Parse cases.tsv. Columns: group<TAB>cpp_test<TAB>rust_test[<TAB>pkg].
    A 4th `pkg` column (default `emf-common`) selects the crate for this row."""
    rows = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if parts[0] == "group":
            continue  # header
        if len(parts) == 4:  # group cpp_test rust_test pkg
            rows.append((parts[0], parts[1], parts[2], parts[3]))
        elif len(parts) == 3:  # group cpp_test rust_test (pkg defaults)
            rows.append((parts[0], parts[1], parts[2], "emf-common"))
    return rows


def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd or ROOT, capture_output=True, text=True)


def oracle(paths):
    """Merge one or more oracle JSON files (later files override dup keys)."""
    merged = {}
    for p in paths:
        merged.update(json.load(open(p, encoding="utf-8")))
    return merged


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

    oracle_paths = [p for p in args.oracle.split(",") if p]
    missing = [p for p in oracle_paths if not os.path.exists(p)]
    if missing:
        sys.exit(f"oracle 缺失: {', '.join(missing)}\n先运行 build_oracle.sh")

    o = oracle(oracle_paths)
    rows = parse_tsv(args.cases)
    names_cache = {}

    def rust_names_of(pkg):
        r = run([CARGO, "test", "-p", pkg, "--", "--list"])
        return {line[: -len(": test")] for line in r.stdout.splitlines() if line.endswith(": test")}

    def rust_names(pkg):
        if pkg not in names_cache:
            names_cache[pkg] = rust_names_of(pkg)
        return names_cache[pkg]

    mapped = [(g, c, rt, pk) for g, c, rt, pk in rows if rt]
    by_cpp = {(c): (g, rt, pk) for g, c, rt, pk in mapped}

    pending = 0
    passed = 0
    regress = []

    for cpp, status in sorted(o.items()):
        if cpp in by_cpp:
            g, rt, pk = by_cpp[cpp]
            if status != "pass" or rt not in rust_names(pk):
                pending += 1
                verdict = "PENDING"
                note = "oracle未通过" if status != "pass" else f"Rust测试缺失: {rt} ({pk})"
            else:
                if rust_pass_any(pk, rt):
                    passed += 1
                    verdict = "PASS"
                    note = f"{rt} ({pk})"
                else:
                    regress.append((g, cpp, rt, pk))
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
        for g, c, rt, pk in regress:
            print(f"  {g} :: {c}  <- Rust {rt} ({pk}) 失败")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())