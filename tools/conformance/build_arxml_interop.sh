#!/usr/bin/env bash
# 构建双向 ARXML 互操作 harness 的 C++ 半侧
# （tools/conformance/interop_arxml_main.cpp），链接 artop-cpp 参考实现。
# 产物：build/interop_arxml。
#
# 复用 build_artop_runtime_oracle.sh 产出的 build/libemf_artop_runtime_modules.a
# （含 emf-artop-runtime / emf-xmi / emf-ecore-codegen / emf-ecore / emf-ecore-util
# / emf-common 全部模块对象）；若不存在则就地编译模块归档。
set -euo pipefail

ARTOP="${1:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-artop/emf-artop-runtime}"
XMI="${2:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-xmi}"
CODEGEN="${3:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-codegen}"
ECORE="${4:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore}"
UTIL="${5:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-ecore-util}"
COMMON="${6:-/tmp/artop-cpp-ref/cpp/emf-cpp/emf-common}"
OUT="${7:-$(cd "$(dirname "$0")" && pwd)/build}"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"

# `$ECORE/include` 必须先于 `$UTIL/include`（emf-ecore-util 自带不兼容的
# ConversionDelegate.h）；emf-artop-runtime 的 arxml 读写需要 pugixml 头。
INCLUDES=(
  -I"$ARTOP/include" -I"$ARTOP/model"
  -I"$XMI/include" -I"$XMI/third-party/pugixml"
  -I"$CODEGEN/include" -I"$ECORE/include" -I"$COMMON/include" -I"$UTIL/include"
)

LIB="$OUT/libemf_artop_runtime_modules.a"
if [[ ! -f "$LIB" ]]; then
  echo "module archive missing; building it (run build_artop_runtime_oracle.sh first to reuse)"
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
  OBJ="$OUT/obj_arxml_interop"; rm -rf "$OBJ"; mkdir -p "$OBJ"
  objs=()
  i=0
  for f in "${SRCS[@]}"; do
    o="$OBJ/mod_$i.o"; i=$((i + 1))
    g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" -c "$f" -o "$o"
    objs+=("$o")
  done
  rm -f "$LIB"
  ar rcs "$LIB" "${objs[@]}"
fi

g++ -std=c++17 -O1 -pthread "${INCLUDES[@]}" \
    "$HERE/interop_arxml_main.cpp" "$LIB" -o "$OUT/interop_arxml"
echo "built $OUT/interop_arxml"