#!/usr/bin/env bash
# Build the C++ emf-artop-runtime reference (oracle) binary and run it,
# producing build/artop_runtime_oracle.json.
#
# Mirrors the C++ CMake `emf_artop_runtime_tests` target: the artop-runtime
# module links emf-common / emf-ecore / emf-xmi / emf-ecore-codegen. The
# module's `registerDefaultAutosar40Metamodel` reads the emitted
# `model/autosar40.ecore`, whose absolute path is injected via the
# `EMF_ARTOP_AUTOSAR40_ECORE` compile definition (see the module CMakeLists).
#
# The artop-runtime tests use the self-contained EMF_TEST mini framework
# (EMF_RUN), whose output lines are "  [ RUN ] name" / "  [ OK  ] name" /
# "  [FAIL ] name", so the log parser below differs from the gtest-based
# oracles.
set -euo pipefail

ARTOP="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-artop/emf-artop-runtime}"
XMI="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xmi}"
CODEGEN="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-codegen}"
ECORE="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
UTIL="${5:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-util}"
COMMON="${6:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${7:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRCS=(
  "$ARTOP"/src/*.cpp
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
TESTS=("$ARTOP"/tests/*.cpp)

# `$ECORE/include` must precede `$UTIL/include` (emf-ecore-util ships its own
# incompatible ConversionDelegate.h). emf-artop-runtime needs pugixml headers
# (bundled under emf-xmi/third-party) for AutosarXMLLoader/Saver.
INCLUDES=(
  -I"$ARTOP/include" -I"$ARTOP/model"
  -I"$XMI/include" -I"$XMI/third-party/pugixml"
  -I"$CODEGEN/include" -I"$ECORE/include" -I"$COMMON/include" -I"$UTIL/include"
)

# Inject the built-in autosar40.ecore path the module's
# registerDefaultAutosar40Metamodel() reads at runtime.
DEFS=(-DEMF_ARTOP_AUTOSAR40_ECORE="\"$ARTOP/model/autosar40.ecore\"")

# The C++ modules are STATIC libraries in CMake, so objects with unresolved
# symbols are not pulled into the test link unless referenced. Reproduce that
# by archiving the module objects and linking the tests against the archive.
OBJ="$OUT/obj_artop"; rm -rf "$OBJ"; mkdir -p "$OBJ"
objs=()
i=0
for f in "${SRCS[@]}"; do
  o="$OBJ/mod_$i.o"; i=$((i + 1))
  g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" "${DEFS[@]}" -c "$f" -o "$o"
  objs+=("$o")
done
rm -f "$OUT/libemf_artop_runtime_modules.a"
ar rcs "$OUT/libemf_artop_runtime_modules.a" "${objs[@]}"

bin="$OUT/emf_artop_runtime_tests"
g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" "${DEFS[@]}" \
    "${TESTS[@]}" "$OUT/libemf_artop_runtime_modules.a" -o "$bin"

# Tolerate a non-zero test exit so the JSON is still produced: a failing C++
# reference test is reported as PENDING (not REGRESSION) by compare.py.
"$bin" > "$OUT/artop_runtime_oracle.log" 2>&1 || true

python3 - "$OUT/artop_runtime_oracle.log" "$OUT/artop_runtime_oracle.json" <<'PY'
import json, sys
log, out = sys.argv[1], sys.argv[2]
res, cur = {}, None
for raw in open(log, encoding='utf-8', errors='replace'):
    s = raw.strip()
    if s.startswith('[ RUN ] '):
        cur = s[len('[ RUN ] '):]
    elif s.startswith('[ OK  ] '):
        res[cur] = res.get(cur) or 'pass'
    elif s.startswith('[FAIL ] '):
        res[s[len('[FAIL ] '):].split(':')[0]] = 'fail'
json.dump(res, open(out, 'w'), indent=2, sort_keys=True)
total = len(res); passed = sum(1 for v in res.values() if v == 'pass')
print(f"artop-runtime oracle: {total} tests, {passed} pass -> {out}")
PY