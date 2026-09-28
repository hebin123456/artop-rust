#!/usr/bin/env bash
# Build the C++ emf-edit reference (oracle) binary and run it, producing
# build/edit_oracle.json. Mirrors build_ecore_oracle.sh but for the emf-edit
# C++ module (which links against emf-common + emf-ecore + emf-ecore-util).
set -euo pipefail

EDIT="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-edit}"
ECORE="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
COMMON="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
UTIL="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-util}"
OUT="${5:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRCS=(
  "$EDIT"/src/*.cpp
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$UTIL"/src/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)
TESTS=(
  tests/CommandTests.cpp tests/EditingDomainTests.cpp tests/PlaceholderTests.cpp
  tests/test_main.cpp
)

bin="$OUT/emf_edit_tests"
g++ -std=c++17 -O1 -pthread \
    -I"$EDIT/include" -I"$ECORE/include" -I"$COMMON/include" -I"$UTIL/include" \
    "${SRCS[@]}" "${TESTS[@]/#/$EDIT/}" -o "$bin"
"$bin" > "$OUT/edit_oracle.log" 2>&1

python3 - "$OUT/edit_oracle.log" "$OUT/edit_oracle.json" <<'PY'
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
print(f"edit oracle: {total} tests, {passed} pass -> {out}")
PY