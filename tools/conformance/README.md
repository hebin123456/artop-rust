# 一致性测试框架 —— C++ 与 Rust 行为等价

目标：Rust 移植的每个行为，都能证明与 C++ 参考实现**等价**。方法就是用户要求的
“直接取 C++ 的二进制来跑”：把 artop-cpp 里对应的 C++ 单测二进制作 oracle，跑出参考
结果；Rust 侧跑同名/对应测试，逐条比对。

一开始缺失实现跑不过很正常；框架把每条行为标成 `PASS / PENDING / REGRESSION`，
只有 `REGRESSION`（已映射但 Rust 失败）才让脚本非零退出。实现没到位的 `PENDING`
不影响仓库健康，随着逐个模块落地逐步收敛为全 `PASS`。

## 组成

| 文件 | 作用 |
|---|---|
| `build_oracle.sh` | 编译并运行 C++ 参考二进制（emf-common 单测），产出 `build/oracle.json` + `build/oracle.log` |
| `cases.tsv` | C++ 测试 ↔ Rust 测试 等价映射表（`group<TAB>cpp_test<TAB>rust_test`） |
| `compare.py` | 跑 oracle + 逐条跑 Rust 测试，输出 `PASS/PENDING/REGRESSION` 汇总 |

## 用法

```sh
# 1. 编译并运行 C++ oracle（给出本机 artop-cpp 的 emf-common 路径）
tools/conformance/build_oracle.sh <path-to-artop-cpp>/cpp/emf-cpp/emf-common

# 2. 对比
python3 tools/conformance/compare.py
```

`compare.py` 还接受 `--oracle <path>` 与 `--pkg <crate>`。

## 结论含义

| 结论 | 含义 | 是否阻断 |
|---|---|---|
| `PASS` | C++ 通过 且 Rust 对应测试通过 | 否 |
| `PENDING` | 未映射，或 Rust 对应测试还没实现 | 否 |
| `REGRESSION` | 已映射但 Rust 对应测试失败 | 是（退出码 1） |

## 逐步补齐节奏

1. 拿当前 C++ 单测清单（oracle）作为待办基线。
2. 每次实现某个 Rust 模块后，在 `cases.tsv` 按 `group` 补上对应行。
3. 跑 `compare.py`，把该行为从 `PENDING` 翻到 `PASS`。
4. 按模块扩 oracle：后续加入 ecore / xmi 的 C++ tests 目录（它们各自有独立 `tests/`）。

## 当前基线（首次接入）

- oracle：`emf-common` C++ 单测，全量 193 条，0 失败。
- 首批复用 `URI_*` 映射到 `emf-common` 的 Rust URI 测试，已验证可在本机跑通 `PASS`。
- 其余（EList / SegmentSequence / EMap / ...）在 `cases.tsv` 中以注释占位，逐步启用。