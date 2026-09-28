#!/usr/bin/env bash
# Build the C++ emf-sphinx reference (oracle) binary and run it, producing
# build/sphinx_oracle.json. Mirrors build_edit_oracle.sh but for the emf-sphinx
# C++ module (which links emf-common + emf-ecore + emf-ecore-util + emf-edit +
# emf-xmi).
set -euo pipefail

SPHINX="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-sphinx}"
XMI="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xmi}"
EDIT="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-edit}"
UTIL="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-util}"
ECORE="${5:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
COMMON="${6:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${7:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

# emf-sphinx sources are collected recursively, mirroring the CMakeLists glob.
# The CMake build also drops files that pull in Eclipse-platform types
# (IWorkspace/IProject/IFile); replicate that filter so the compile is
# hermetic on a headless runner.
SPHINX_SRCS=()
while IFS= read -r f; do
  if head -n 200 "$f" | grep -qE 'IWorkspace|IProject |IFile '; then
    continue
  fi
  SPHINX_SRCS+=("$f")
done < <(find "$SPHINX/src" -name '*.cpp' | sort)

SRCS=(
  "${SPHINX_SRCS[@]}"
  "$XMI"/src/*.cpp
  "$XMI"/third-party/pugixml/pugixml.cpp
  "$EDIT"/src/*.cpp
  "$UTIL"/src/*.cpp
  "$ECORE"/src/*.cpp
  "$ECORE"/src/util/*.cpp
  "$COMMON"/src/*.cpp
  "$COMMON"/src/util/*.cpp
  "$COMMON"/src/command/*.cpp
)
TESTS=("$SPHINX"/tests/*.cpp)

# Include order matters: `emf-ecore-util` ships its own (incompatible)
# `emf/ecore/util/ConversionDelegate.h`, so `$ECORE/include` must come before
# `$UTIL/include` for `emf-ecore/src/util/ConversionDelegate.cpp` to resolve the
# right declaration. Mirrors build_edit_oracle.sh. emf-xmi bundles pugixml under
# `third-party/`, whose directory must be on the include path.
INCLUDES=(
  -I"$SPHINX/include" -I"$SPHINX/src" -I"$XMI/include"
  -I"$XMI/third-party/pugixml" -I"$EDIT/include" -I"$ECORE/include"
  -I"$COMMON/include" -I"$UTIL/include"
)

# The C++ module is a STATIC library in CMake, so object files with unresolved
# symbols are simply not pulled into the test link unless referenced. Reproduce
# that by archiving the module objects and linking the tests against the archive
# (e.g. `ScopingResourceSetImpl.cpp` / `AbstractProxyResolverService.cpp` call
# pure-virtual base members that no test triggers).
OBJ="$OUT/obj"; rm -rf "$OBJ"; mkdir -p "$OBJ"
objs=()
i=0
for f in "${SRCS[@]}"; do
  o="$OBJ/mod_$i.o"; i=$((i + 1))
  g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" -c "$f" -o "$o"
  objs+=("$o")
done
rm -f "$OUT/libemf_sphinx_modules.a"
ar rcs "$OUT/libemf_sphinx_modules.a" "${objs[@]}"

bin="$OUT/emf_sphinx_tests"
g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" \
    "${TESTS[@]}" "$OUT/libemf_sphinx_modules.a" -o "$bin"
# Tolerate a non-zero test exit so the JSON is still produced: a failing C++
# reference test is reported as PENDING (not REGRESSION) by compare.py.
"$bin" > "$OUT/sphinx_oracle.log" 2>&1 || true

python3 - "$OUT/sphinx_oracle.log" "$OUT/sphinx_oracle.json" <<'PY'
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
print(f"sphinx oracle: {total} tests, {passed} pass -> {out}")
PY