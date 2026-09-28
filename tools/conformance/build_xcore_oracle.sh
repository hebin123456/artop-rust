#!/usr/bin/env bash
# Build the C++ emf-xcore reference (oracle) binary and run it, producing
# build/xcore_oracle.json. Mirrors build_acceleo_oracle.sh but for the emf-xcore
# module itself (which links against emf-common + emf-ecore).
set -euo pipefail

XCORE="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xcore}"
ECORE="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
COMMON="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${4:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRCS=(
  "$XCORE"/src/*.cpp
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)
TESTS=(
  tests/XcoreTests.cpp tests/test_main.cpp
)

bin="$OUT/emf_xcore_tests"
g++ -std=c++17 -O1 -pthread \
    -I"$XCORE/include" -I"$ECORE/include" -I"$COMMON/include" \
    "${SRCS[@]}" "${TESTS[@]/#/$XCORE/}" -o "$bin"
# Tolerate a non-zero test exit so the JSON is still produced: a failing C++
# reference test is reported as PENDING (not REGRESSION) by compare.py.
"$bin" > "$OUT/xcore_oracle.log" 2>&1 || true

python3 - "$OUT/xcore_oracle.log" "$OUT/xcore_oracle.json" <<'PY'
import json, sys
log, out = sys.argv[1], sys.argv[2]
res, cur = {}, None
for line in open(log, encoding='utf-8', errors='replace'):
    line = line.rstrip('\n')
    if line.startswith('[ RUN      ] '):
        cur = line[len('[ RUN      ] '):]
    elif line.startswith('[       OK ] '):
        res[cur] = res.get(cur) or 'pass'
    elif line.startswith('[  FAILED  ] '):
        res[line[len('[  FAILED  ] '):].split(':')[0]] = 'fail'
json.dump(res, open(out, 'w'), indent=2, sort_keys=True)
total = len(res); passed = sum(1 for v in res.values() if v == 'pass')
print(f"xcore oracle: {total} tests, {passed} pass -> {out}")
PY