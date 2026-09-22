#!/usr/bin/env bash
# Build the C++ emf-ecore reference (oracle) binary and run it, producing
# build/ecore_oracle.json. Mirrors build_oracle.sh but for the emf-ecore C++
# module (which links against emf-common headers).
set -euo pipefail

ECORE="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
COMMON="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${3:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRCS=(
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)
TESTS=(
  tests/BasicEObjectTests.cpp tests/ChangeNotificationTests.cpp tests/DataTypeUtilTests.cpp
  tests/DynamicEObjectImplTests.cpp tests/EClassImplTests.cpp tests/EObjectEInvokeTests.cpp
  tests/EPackageImplTests.cpp tests/ETypedElementImplTests.cpp tests/EcorePackageTests.cpp
  tests/test_main.cpp
)

bin="$OUT/emf_ecore_tests"
g++ -std=c++17 -O1 -pthread -I"$ECORE/include" -I"$COMMON/include" \
    "${SRCS[@]}" "${TESTS[@]/#/$ECORE/}" -o "$bin"
"$bin" > "$OUT/ecore_oracle.log" 2>&1

python3 - "$OUT/ecore_oracle.log" "$OUT/ecore_oracle.json" <<'PY'
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
print(f"ecore oracle: {total} tests, {passed} pass -> {out}")
PY