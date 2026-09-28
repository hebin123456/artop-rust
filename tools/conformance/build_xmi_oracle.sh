#!/usr/bin/env bash
# Build the C++ emf-xmi reference (oracle) binary and run it, producing
# build/xmi_oracle.json. Mirrors the C++ CMake `emf_xmi_tests` target, which
# links emf-xmi (+ emf-ecore-codegen) over emf-common / emf-ecore / emf-ecore-util.
set -euo pipefail

XMI="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xmi}"
CODEGEN="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-codegen}"
ECORE="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
UTIL="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-util}"
COMMON="${5:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${6:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

# Mirror the CMake target's source list (probes / roundtrip_test are separate
# executables and are intentionally not part of emf_xmi_tests).
TESTS=(
  "$XMI"/tests/test_main.cpp
  "$XMI"/tests/XMISaverTests.cpp
  "$XMI"/tests/XMILoaderTests.cpp
  "$XMI"/tests/XMIResourceFactoryTests.cpp
  "$XMI"/tests/RoundtripTests.cpp
  "$XMI"/tests/JavaInteropTests.cpp
  "$XMI"/tests/XMLHelperTests.cpp
  "$XMI"/tests/P3_XMLSaveLoadUUIDTests.cpp
  "$XMI"/tests/P3_5_GetEObjectByIDHrefTests.cpp
  "$XMI"/tests/E2E_MultiFileEcoreTests.cpp
  "$XMI"/tests/E2E_GenModelXmiTests.cpp
  "$XMI"/tests/E2E_GenModelXmiTypedMultiFileTests.cpp
  "$XMI"/tests/E2E_GenModelXmiCxxProducesTests.cpp
  "$XMI"/tests/E2E_GenModelXmiMultiEcoreTypedTests.cpp
  "$XMI"/tests/E2E_GenModelXmiEquivalentReplacementTests.cpp
  "$XMI"/tests/E2E_ProxyModelTests.cpp
  "$XMI"/tests/RuntimeBehaviorTests.cpp
  "$XMI"/tests/StaticVsDynamicXmiTests.cpp
  "$XMI"/tests/XmiInteropTests.cpp
)

SRCS=(
  "$XMI"/src/*.cpp
  "$XMI"/third-party/pugixml/pugixml.cpp
  "$CODEGEN"/src/*.cpp
  "$UTIL"/src/*.cpp
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)

# `$ECORE/include` must precede `$UTIL/include`: emf-ecore-util ships its own
# (incompatible) `emf/ecore/util/ConversionDelegate.h`. emf-xmi bundles pugixml
# under `third-party/`.
INCLUDES=(
  -I"$XMI/include" -I"$XMI/third-party/pugixml" -I"$XMI/tests"
  -I"$CODEGEN/include" -I"$ECORE/include" -I"$COMMON/include" -I"$UTIL/include"
)

# The C++ modules are STATIC libraries in CMake, so objects with unresolved
# symbols are not pulled into the test link unless referenced. Archive the
# module objects and link the tests against the archive to reproduce that.
OBJ="$OUT/obj_xmi"; rm -rf "$OBJ"; mkdir -p "$OBJ"
objs=()
i=0
for f in "${SRCS[@]}"; do
  o="$OBJ/mod_$i.o"; i=$((i + 1))
  g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" -c "$f" -o "$o"
  objs+=("$o")
done
rm -f "$OUT/libemf_xmi_modules.a"
ar rcs "$OUT/libemf_xmi_modules.a" "${objs[@]}"

# Emulate the CMake compile definitions the E2E tests rely on at runtime.
EMFCPP_SOURCE_DIR="$(cd "$XMI/.." && pwd)"
DEFS=(
  -DEMF_CODEGEN_TEST_OUTPUT_DIR="\"$OUT/emf-xmi-tests\""
  -DEMF_BUILD_DIR="\"$OUT\""
  -DEMFCPP_SOURCE_DIR="\"$EMFCPP_SOURCE_DIR\""
  -DEMF_TEST_CXX_FLAGS="\"-std=c++17\""
)

bin="$OUT/emf_xmi_tests"
g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" "${DEFS[@]}" \
    "${TESTS[@]}" "$OUT/libemf_xmi_modules.a" -o "$bin"

# Tolerate a non-zero test exit so the JSON is still produced: a failing C++
# reference test is reported as PENDING (not REGRESSION) by compare.py.
"$bin" > "$OUT/xmi_oracle.log" 2>&1 || true

python3 - "$OUT/xmi_oracle.log" "$OUT/xmi_oracle.json" <<'PY'
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
print(f"xmi oracle: {total} tests, {passed} pass -> {out}")
PY