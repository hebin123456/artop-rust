#!/usr/bin/env bash
# Build the C++ emf-acceleo reference (oracle) binary and run it, producing
# build/acceleo_oracle.json. Mirrors build_edit_oracle.sh but for the emf-acceleo
# C++ module (which links against emf-common + emf-ecore + emf-xcore).
set -euo pipefail

ACCELEO="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-acceleo}"
XCORE="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xcore}"
ECORE="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
COMMON="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${5:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRCS=(
  "$ACCELEO"/src/*.cpp
  "$XCORE"/src/*.cpp
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)
TESTS=(
  tests/AcceleoTests.cpp tests/AlignmentTests.cpp tests/test_main.cpp
)

bin="$OUT/emf_acceleo_tests"
g++ -std=c++17 -O1 -pthread \
    -I"$ACCELEO/include" -I"$XCORE/include" -I"$ECORE/include" -I"$COMMON/include" \
    "${SRCS[@]}" "${TESTS[@]/#/$ACCELEO/}" -o "$bin"
"$bin" > "$OUT/acceleo_oracle.log" 2>&1

python3 - "$OUT/acceleo_oracle.log" "$OUT/acceleo_oracle.json" <<'PY'
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
print(f"acceleo oracle: {total} tests, {passed} pass -> {out}")
PY