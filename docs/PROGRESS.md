# artop-rust 移植进度记录

> 本文档跟踪把一个 C++ 实现的 EMF + ARTOP 模型平台（[`hebin123456/artop-cpp`](https://github.com/hebin123456/artop-cpp)）1:1 移植到 Rust 的整体进度。
> 更新策略：每个可验证的里程碑（模块可用 / 测试全绿 / 提交）在此登记。

## 1. 目标与底层思路

- 目标：把每个 `emf-*` C++ 模块做成**行为等价**的 Rust crate；artop（AUTOSAR）专属层在此基础上叠加。
- 反射与继承：用**元数据**（`eSuperTypes` 图 + FeatureID）表达继承，而非 Rust 类型继承。这样 1925 类的 AUTOSAR 元模型也能秒级编译。通用算法通过 `&dyn` / 枚举分派。
- 行为等价：Rust 与 C++ 用同一套输入（`.arxml`），产出可对比的结果（诊断、序列化、反射查询）。见 §5 的一致性测试框架。

## 2. 目录与命名（已对齐 C++ 布局）

原 C++ 布局：`cpp/emf-cpp/emf-<module>`，artop 专属放在 `cpp/emf-cpp/emf-artop/` 下。

Rust 工作区对应：

```
crates/
  emf-common        <- emf-common
  emf-ecore         <- emf-ecore
  emf-ecore-util    <- emf-ecore-util
  emf-ecore-codegen <- emf-ecore-codegen
  emf-xmi           <- emf-xmi
  emf-xsd           <- emf-xsd
  emf-edit          <- emf-edit
  emf-compare       <- emf-compare
  emf-validation    <- emf-validation
  emf-xcore         <- emf-xcore
  emf-acceleo       <- emf-acceleo
  emf-sphinx        <- emf-sphinx
  emf-artop/
    autosar448-model  <- emf-artop/autosar448-model（生成的 AUTOSAR 4.4.8 注册表）
    artop-runtime     <- emf-artop/artop-runtime
    artop-codegen     <- emf-artop/artop-codegen
examples/
  arxml-roundtrip   <- examples/arxml_roundtrip
  arxml-validate    <- examples/arxml_validate
```

上次提交（`70ff898`）完成命名重构：EMF 基础库统一为 `emf-*`，artop 专属归入 `crates/emf-artop/`。

## 3. 模块状态总览

| 模块 | 状态 | 说明 |
|---|---|---|
| `emf-common` | ✅ 工作 | URI / EList / Notifier / Notification / FeatureMap / Resource / ResourceSet / EPackageRegistry / Command / EMap / Val / Diagnostic / SegmentSequence / URIConverter |
| `emf-ecore` | ✅ 工作 | EClass / EStructuralFeature / EAttribute / EReference / EOperation / EParameter / EPackage / EFactory / EEnum / EDataType / DynamicEObject / EcorePackage / FeatureID 常量 |
| `emf-artop/autosar448-model` | ✅ 工作 | 生成的 AUTOSAR 4.4.8 注册表 + 反射查询（eAllFeatures / isSuperTypeOf / eGet） |
| `emf-ecore-util` | ⬜ 骨架 | EcoreUtil / Copier / EMap / validator 等 |
| `emf-ecore-codegen` | ⬜ 骨架 | GenModel → 代码生成 |
| `emf-xmi` | ⬜ 骨架 | XMI/XML 序列化、代理、UUID（核心，优先推进） |
| `emf-xsd` | ⬜ 骨架 | XSD 元模型 |
| `emf-edit` | ⬜ 骨架 | 命令 / 编辑域 |
| `emf-compare` | ⬜ 骨架 | match + diff + merge |
| `emf-validation` | ⬜ 骨架 | 批量 + 实时校验 |
| `emf-xcore` | ⬜ 骨架 | Xcore DSL |
| `emf-acceleo` | ⬜ 骨架 | MTL / M2T |
| `emf-sphinx` | ⬜ 骨架 | headless 核心 |
| `emf-artop/artop-runtime` | ⬜ 骨架 | AUTOSAR 序列化 / 反序列化 / 版本元数据 |
| `emf-artop/artop-codegen` | ⬜ 骨架 | `.ecore` → 静态模型 |

## 4. emf-common / emf-ecore 实现要点（已完成）

- **`Val`**：替换 C++ `std::any` 的类型安全值信封。原子（Bool/Int/Long/Double/String/…）、数组、对象引用（`Rc<RefCell<dyn EObject>>`）。实现自定义 `PartialEq`（对象按 `ptr_eq`）。
- **`EList` / `EMap`**：带变更追踪（`on_change: FnMut(ListChange)`）与按键索引的列表。含克隆语义（有闭包时手动实现 Debug/Clone）。
- **`URI` / `SegmentSequence`**：完整解析、规范化、相对解析、fragment 处理；`is_relative()` 语义对齐 EMF。序列化统一走 `Display`（已消除遮蔽 `Display` 的固有 `to_string`）。
- **`Resource` / `ResourceSet`**：URI 编址的模型持久化单元，`e_resource()` 沿包容树向上解析。
- **`EcorePackage`**：线程局部全局注册表（`thread_local!` + `RefCell`），内建 `EString`/`EInt`/… 数据类型 + 20 个 meta-类。提供 `datatype::from_string`/`to_string` 字面量互相转换。
- **`EClass` 继承**：元数据式 `eSuperTypes` 名表 + 祖先优先遍历；`e_all_structural_features` 按 FeatureID 去重；`e_all_attributes` / `e_all_references` 拆分；`is_super_type_of` 为严格祖先语义。
- **`DynamicEObject`**：按 FeatureID 存储的可反射对象；`eGet/eSet/eIsSet/eUnset`；支持绑定额外注册表（`new_in`/`bind_registry`）以解析继承特征。
- **`EFactory`**：`create(EClass)` 实例化 `DynamicEObject`；`createFromString` / `convertToString` 处理内建数据类型与枚举。

### 本阶段亮点修复
- `DynamicEObject` 继承特征解析从"全局注册表"改为"归属注册表优先 + 全局兜底"，修复 `shortName` 等继承属性查不到的问题（XMI 加载反射建对象的必要前提）。
- 清理全部编译/裁剪警告：未用导入、死代码、固有 `to_string` 遮蔽 `Display`、`non_snake_case` 等。

## 5. 与 C++ 等价的一致性测试框架（进行中）

设计：**同一份输入，C++ 与 Rust 各跑一遍，对比结构输出**，允许逐步补齐实现，暂未实现时先标记 SKIP，最终收敛为全过。

- 编译 C++ 参考实现（直接取 C++ 二进制，见 `compat-cpp/` 与 `scripts/`），产出参考输出。
- Rust 侧用对应测试用例产出结果。
- 比对器做归一化（忽略顺序、时间戳、平台差异）后逐个用例比对。
- 覆盖：URI 规范化、EClass 继承/反射、FeatureMap、XMI roundtrip、诊断序列等。

TODO（见 §7）：
- [ ] 搭好 C++ 构建入口与用例数据目录
- [ ] 比对器 skeleton
- [ ] 首批用例：URI + Ecore 反射
- [ ] XMI loader/saver roundtrip 等价
- [ ] CI 集成：C++ 参考输出随 CI 缓存，Rust 用例与之对比

## 6. 质量门禁（每次提交前）

```sh
cargo fmt    --all -- --check
cargo test   --workspace --all-targets     # 当前 18 个测试套件全绿
cargo clippy --workspace --all-targets     # 无 error / warning
cargo build  --workspace --release         # release 也通过
```

## 7. 下一步（按优先级）

1. `emf-xmi`：XMI/XML load/save（解析 `.arxml` 的核心依赖），先做 loader + saver roundtrip + 诊断。
2. `emf-ecore-util`：Copier / EcoreUtil / eContents / eCrossReferences。
3. 补 `Val` 对**对象引用、多值**、eIsSet/eUnset、内容树与交叉引用的完整表达。
4. `artop-runtime`：AUTOSAR 序列化/反序列化。
5. 一致性测试框架扩展到 XMI 用例并接入 CI。

## 8. 提交记录（与本仓库进度相关的近期提交）

| 提交 | 内容 |
|---|---|
| `70ff898` | 命名重构：EMF 基础库 `emf-*`，artop 专属入 `crates/emf-artop/` |
| `df146ec` | 完成 `emf-common` + `emf-ecore` EMF 基础层（完整实现 + 集成测试） |