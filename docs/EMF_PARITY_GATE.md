# EMF 底座对齐门线（HARD GATE）

> 本文件是一条**硬性门线**，任何开发都必须遵守。

## 门线规则

1. **必须先完成 `emf-*` 全部 EMF 通用底座的能力对齐（与 artop-cpp 的 C++ 测试逐条对上），在此之后才有资格动手 artop。**
2. **在底座对齐完成前，绝对不许考虑 / 触碰任何 artop 相关内容**：
   - 不得实现、修改、讨论 `artop-runtime` / `artop-codegen` / `autosar448-model` / `arxml` 等 artop 专属层。
   - 不得在 `emf-*` 模块里写入 artop / AUTOSAR 相关代码、注释、导出符号。
   - 代码审查与提交都不允许混入 artop 改动。
3. 门线判定依据：`docs/PARITY_TRACKER.md` 中的每一条 C++ 测试文件对应的 Rust 对照测试**全绿**。

## 判定状态

| 门线 | 状态 |
|---|---|
| EMF 底座全部 C++ 测试对照移植并跑绿 | ✅ 已完成 |
| 允许接触 artop | ✅ 允许 |

### 判定依据

- `emf-ecore`：C++ oracle 共 **153** 条（`tools/conformance/build/ecore_oracle.json`，153 pass），
  `tools/conformance/cases_ecore.tsv` 已 **153/153 全映射**，`compare.py` 结果为
  **等价(PASS) 153、待实现(PENDING) 0、REGRESSION 0**。
- 其余底座模块 oracle 亦为 0 PENDING 全 PASS：`emf-common`（含 command / NotifyingList /
  SegmentSequence / UniqueEList / ENotifier / EAdapter）、`emf-edit`(26)、`emf-acceleo`(26)、
  `emf-sphinx`(68)；artop 侧 `artop-runtime`(18) 同步对齐（见 `docs/PARITY_TRACKER.md`）。
- 曾记作“不可 1:1 移植”的 2 条 emf-ecore 用例已如实表达并转 PASS：
  `BasicEObject_EClass_DefaultNull`（裸 `EObject` 的 `e_class()` 返回空名表达“无分类器”）、
  `RemoveAdapter_DuringNotify_SafeIteration`（`remove_adapter` 已对齐 C++/Java
  `eBasicRemoveAdapter`：先 `eNotify` 再 `erase`）。

> 门线已解除，artop 层（`artop-runtime` / `artop-codegen` / `autosar448-model` /
> `artop-validation`）允许开发与发布。第 2 条的分层纪律在解除后仍保留：artop 专属代码
> 只能出现在 `crates/emf-artop/`，不得回灌进 `emf-*` 底座。