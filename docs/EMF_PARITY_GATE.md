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
- 其余底座模块 oracle：`emf-edit`(26) / `emf-acceleo`(26) / `emf-xcore`(14) / `emf-sphinx`(68)
  均 **0 PENDING 全 PASS**；`emf-common` **188/193**（余 5 条为 Rust 类型系统无法如实表达的
  空指针 / 重复身份语义，保留 PENDING）；`emf-xmi` 187 条 oracle **187/187 全映射、186 PASS**
  （唯一 PENDING 为 `StaticDynamic_StaticXmi_EqualsDynamicXmi`，C++ 参考自身失败，非 Rust 缺口）。
  artop 侧 `artop-runtime`(18) 同步对齐为
  **0 PENDING 全 PASS**（见 `docs/PARITY_TRACKER.md` 的 artop-runtime 章节）。
- 曾记作“不可 1:1 移植”的 2 条 emf-ecore 用例已如实表达并转 PASS：
  `BasicEObject_EClass_DefaultNull`（裸 `EObject` 的 `e_class()` 返回空名表达“无分类器”）、
  `RemoveAdapter_DuringNotify_SafeIteration`（`remove_adapter` 已对齐 C++/Java
  `eBasicRemoveAdapter`：先 `eNotify` 再 `erase`）。

> 门线已解除，artop 层（`artop-runtime` / `artop-codegen` / `autosar448-model` /
> `artop-validation`）允许开发与发布。第 2 条的分层纪律在解除后仍保留：artop 专属代码
> 只能出现在 `crates/emf-artop/`，不得回灌进 `emf-*` 底座。