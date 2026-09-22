#!/usr/bin/env bash
# Build the C++ reference (oracle) binary and run it, producing oracle.json.
#
# The oracle is the actual emf-common C++ unit-test binary from artop-cpp
# (sources + self-contained EMF_TEST framework, no external test deps).
# "取 C++ 二进制来跑" 即指此: 每次收敛都以其通过/失败为准。
#
# Usage:
#   build_oracle.sh <path-to-artop-cpp>/cpp/emf-cpp/emf-common   [outdir]
#
# Produces (in outdir, default ./build):
#   oracle.json   { "<test name>": "pass"|"fail" }     -- oracle 结果
#   oracle.log    C++ 二进制原始输出
set -euo pipefail

COMMON="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${2:-$(cd "$(dirname "$0")" && pwd)/build}"
mkdir -p "$OUT"

SRC=( src/URI.cpp src/Resource.cpp src/EPackageRegistry.cpp src/Diagnostic.cpp \
      src/ENotifier.cpp src/EObject.cpp src/FeatureMap.cpp src/URIConverter.cpp \
      src/util/SegmentSequence.cpp src/util/BasicEList.cpp src/util/BasicEMap.cpp \
      src/util/UniqueEList.cpp src/util/ArrayDelegatingEList.cpp src/util/DelegatingEList.cpp \
      src/util/NotifyingList.cpp src/util/NotifyingListImpl.cpp \
      src/command/AbstractCommand.cpp src/command/CompoundCommand.cpp \
      src/command/UnexecutableCommand.cpp src/command/IdentityCommand.cpp \
      src/command/CommandWrapper.cpp src/command/StrictCompoundCommand.cpp \
      src/command/BasicCommandStack.cpp )
TESTS=( tests/BasicEMapTests.cpp tests/CommandTests.cpp tests/EListTests.cpp \
        tests/ENotifierTests.cpp tests/NotifyingListTests.cpp tests/ResourceTests.cpp \
        tests/SegmentSequenceTests.cpp tests/URITests.cpp tests/UniqueEListTests.cpp \
        tests/test_main.cpp )

bin="$OUT/emf_common_tests"
g++ -std=c++17 -O1 -pthread -I"$COMMON/include" "${SRC[@]/#/$COMMON/}" \
    "${TESTS[@]/#/$COMMON/}" -o "$bin"
"$bin" > "$OUT/oracle.log" 2>&1

python3 - "$OUT/oracle.log" "$OUT/oracle.json" <<'PY'
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
print(f"oracle: {len(res)} tests -> {out}  (all-pass={all(v=='pass' for v in res.values())})")
PY