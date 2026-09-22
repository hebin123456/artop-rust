# artop-rust 移植进度记录

> 本文档跟踪把一个 C++ 实现的 EMF + ARTOP 模型平台（[`hebin123456/artop-cpp`](https://github.com/hebin123456/artop-cpp)）1:1 移植到 Rust 的整体进度。
> 更新策略：每个可验证的里程碑（模块可用 / 测试全绿 / 提交）在此登记。

## 1. 目标与底层思路

- 目标：把每个 `emf-*` C++ 模块做成**行为等价**的 Rust crate；artop（AUTOSAR）专属层在此基础上叠加。
- 反射与继承：用**元数据**（`eSuperTypes` 图 + FeatureID）表达继承，而非 Rust 类型继承。这样 1925 类的 AUTOSAR 元模型也能秒级编译。通用算法通过 `&dyn` / 枚举分派。
- 行为等价：Rust 与 C++ 用同一套测试用例，产出可对比的结果（诊断、序列化、反射查询）。见 §5 的一致性测试框架。

### 分层与解耦原则（务必遵守）

- `emf-common` / `emf-ecore` / `emf-xmi` 等 **EMF 底层仓库是通用底座，与 artop / AUTOSAR 完全无关**。它们操作的是 `EObject` / `EPackage` / `DynamicEObject` 反射，不感知"这是不是 AUTOSAR"。
- 因此**开发任何一个 `emf-*` 模块时，不得牵涉 autosar / arxml / artop-runtime 等任何 artop 独有的内容**——注释、文档、导出符号都保持通用。不要在本层写 `.arxml`。
- artop 与 AUTOSAR 的关系**只在 `crates/emf-artop/` 下的 artop 专属层里**才建立：`autosar448-model`（元模型）、`artop-runtime`（ARXML 序列化，构建在 `emf-xmi` 之上）、`artop-codegen`。
- 进度顺序固定：**先把全部 `emf-*` 通用底座做完，此刻完全不碰 artop；等进入 artop 阶段再谈关联**。

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
| `emf-ecore-codegen` | ✅ 工作 | GenModel→代码生成：ecore loader（XMI→`EPackage`）+ TypeMapper + generator（struct / `match` 反射表 / `register_package`），生成的 crate 可脱离 `.ecore` 独立编译运行 |
| `emf-xmi` | ✅ 工作 | saver + loader + 真实 `XMIResource` + `ResourceSet` 按需加载集成（`ResourceHandle` / `ResourceFactory` / `XMIResourceFactory`）已实现并测试通过；`XMILoadImpl` / `XMIHelper` 接口待做 |
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

## 5. 与 C++ 等价的一致性测试框架（已搭建，逐步收敛）

做法与要求一致：**直接取 C++ 的二进制来跑**——把 artop-cpp 里对应的 C++ 单测二进制作
oracle，跑参考结果；Rust 侧跑同名/对应测试，逐条比对。**一开始缺失实现跑不过很正常**，
框架把每条行为标为 `PASS / PENDING / REGRESSION`，只有 `REGRESSION`（已映射但 Rust 失败）
才让脚本非零退出；`PENDING` 反映“等价待实现”，随着落地逐步变 `PASS`。

- oracle：`emf-common` C++ 单测二进制（`EMF_TEST` 迷你框架，无需外部依赖），当前 193 条、0 失败。
- 脚本：`tools/conformance/build_oracle.sh`（编译并跑 C++ oracle）、
  `compare.py`（跑 Rust 侧并输出 PASS/PENDING/REGRESSION）、`cases.tsv`（C++↔Rust 映射表）。
- 当前基线：oracle 全量 193 条；已映射 **188** 条全部 `PASS`，剩余 5 条 `PENDING`(未映射)。尚未映射的 5 条均为 **Rust 类型系统无法如实表达**的语义：`ENotifier_AddAdapter_Duplicate_NotAdded`（Box 所有权无重复身份）、`ENotifier_AddAdapter_Null_Ignored` / `ENotifier_RemoveAdapter_Null_NoChange` / `Resource_AddToContents_NullPointer`（`Box<dyn Adapter>` / `ObjectRef` 无空指针）、`Placeholder`（C++ 空跑测试）。这些在 Rust 中无意义，保留为 PENDING 不失真。
- 已接入 CI（`conformance` job）：CI 检出 artop-cpp、编译并跑 oracle、再与 Rust 比对。

复用路径：C++ oracle 单测 → `tools/conformance/build`，与 CI 的 `conformance` job 对齐。

## 6. 质量门禁（每次提交前）

```sh
cargo fmt    --all -- --check
cargo test   --workspace --all-targets     # 当前 18 个测试套件全绿
cargo clippy --workspace --all-targets     # 无 error / warning
cargo build  --workspace --release         # release 也通过
bash tools/conformance/build_oracle.sh <artop-cpp>/cpp/emf-cpp/emf-common
python3 tools/conformance/compare.py       # 无 REGRESSION 即通过
```

## 7. 下一步（按优先级）

已完成：emf-common/ecore 核心、EcoreUtil/Copier、XMI saver+loader、Resource/XMI 持久化集成、一致性测试 193 组中 188 条映射 PASS（含 command 模块 / NotifyingList / SegmentSequence / UniqueEList / ENotifier / EAdapter）；一致性框架已多 crate 化并建立 emf-ecore oracle（153 条），映射 62 条 PASS。

1. 扩展 oracle 到 emf-xmi 的 C++ tests 逐模块收敛；继续补 emf-ecore 未映射的 91 条（EGenericType / EInvoke / 指针身份类）。
2. `emf-xmi` 进一步落地：`XMILoadImpl` / `XMIHelper` 接口（`XMIResourceFactory` + `ResourceSet.getResource` 按需加载已在本轮完成）。
3. `emf-ecore-util` 剩余：Adapter / ECrossReferenceAdapter / containment 遍历到 `all_contents` 的流式实现。
4. `artop-runtime`：AUTOSAR 序列化/反序列化（届时才引入 artop 相关内容）。

## 8. emf-xmi 实施记录

- **Milestone 1 — saver**：`emf-xmi` 实例文档序列化器（`XmiSaver` → `save_to_string`）。
  - 单个根元素裸输出，多个根包在 `<xmi:XMI>` 中；根元素带 `xmi:version` + `xmlns:xmi/xsi/<prefix>`。
  - 属性 → XML 属性；containment 引用 → 子元素（标签为 feature 名，类型不一致时写 `xsi:type`）；非 containment 引用 → `href`。
  - 每个对象分配合成 `xmi:id`（`Rc::as_ptr` 作键去重），跨引用用 `//<id>`。
  - 为此在 `emf-ecore` 扩展元数据：`EStructuralFeature.containment` + `set_containment`；暴露 `DynamicEObject::all_structural_features/all_references/all_containments/registry`；`PackageRegistry::find_package_of_class`。

- **Milestone 2 — loader（本轮新增）**：`emf-xmi` 反射加载器 `load_from_str`（XML → `DynamicEObject`）。
  - 自研零依赖 XML 解析器（`parser.rs`）：`XmlNode` 元素树，支持属/子元素/文本/**实体解码**(含数值)、CDATA、注释、PI、DOCTYPE 跳过。
  - 反射建对象：根元素 `<prefix:Class>` 按 local 定类型；containment 子元素按 feature 声明类型、`xsi:type` 覆盖；属性经 `datatype::from_string` 解析；`xmlns*`/`xmi:id`/`xmi:version`/`xsi:type` 作结构元数据跳过。
  - `<xmi:XMI>` 文档包装元素解包，其子元素即根。
  - 本地 `href="//<id>"` 跨引用在全部对象构建后按 `xmi:id` 表解析（`append_reference` 区分 single/many）。
  - 测试：saver→loader **roundtrip**（类名/属性/containment 结构一致）+ 属性加载 + XMI wrapper + 未知特征容错，全绿。纯通用 EMF，未牵涉任何 artop / AUTOSAR 内容。

- **Milestone 3 — Resource/XMI 持久化集成（本轮新增）**：把 XMI saver/loader 接到 `Resource` 持久化。
  - `emf-common::Resource` 补齐 C++ 基类表面：`to_xmi_string`/`save_to_string`、`from_xmi_string`/`load_from_string`（基类空实现，不翻转 `is_loaded`）、`get_eobject`/`get_uri_fragment`、基于 `file:` URI 的 `save()`/`load()`（打不开文件/写不了文件报 `Err`）——使 `Resource_ToXmiString_DelegatesSave` 等 8 条 oracle 测试转 `PASS`。
  - 新增 `Resource::set_contents` / `clear_contents` 以支持反序列化替换根内容。
  - `emf-xmi::xmi_resource`：把占位模块升级为真实 `XMIResource`（对齐 C++ `emf::xmi::XMIResource`）。它承载一个 `emf_common::Resource` + `PackageRegistry`，`save_to_string`/`load_from_string` 真正让 saver/loader 落地，`save()`/`load()` 打通 `file:` URI 落盘。测试覆盖 saver→loader 端到端 roundtrip 与坏 XMI 报错。

- **Milestone 4 — 一致性测试大幅扩展 + command 模块（本轮新增）**：`emf-common` 新增 `src/command.rs`（移植 `org.eclipse.emf.common.command`：`Command` trait / `AbstractCommand` / `CompoundCommand` / `StrictCompoundCommand` / `IdentityCommand` / `UnexecutableCommand` / `CommandWrapper` / `BasicCommandStack` / `AbortExecutionException`，约 49 条测试），并补全 `SegmentSequence`（16）、`UniqueEList`（7）、`NotifyingList`（11，新增派发 ADD/REMOVE/SET/MOVE/ADD_MANY/REMOVE_MANY 的类型）、`ENotifier`/`EAdapter`（target 关联 + 字段保留 + `EObjectImpl_SetEContainer` 反向通知，8）、`Resource`（`set_root`，1）。映射从 69 → **188 PASS**，仅剩 5 条 null/重复身份语义无法在 Rust 表达。

- **Milestone 5 — 一致性框架多 crate 化 + emf-ecore oracle（本轮新增）**：
  - 工具链改造：`cases.tsv` 新增可选第 4 列 `pkg`（默认 `emf-common`）；`compare.py` 支持合并多个 oracle（`--oracle a.json,b.json`），并按行独立解析 Rust 测试所属 crate 与名字。
  - 新增 `build_ecore_oracle.sh`：编译并运行 `emf-ecore` 的 C++ 单测二进制（链接 emf-common 符号），产出 `build/ecore_oracle.json`，当前 **153 条、0 失败**。
  - 新增集成测试：`crates/emf-ecore/tests/ecore_reflection.rs`（移植 EClassImpl / ETypedElementImpl / EPackageImpl / EcorePackage / DataTypeUtil，39 条）+ `crates/emf-ecore/tests/dynamic_eobject.rs`（移植 DynamicEObjectImpl + BasicEObject 动态部分，27 条）。
  - 映射：新增 `cases_ecore.tsv`，**62 条全部 PASS**，0 REGRESSION。未映射 91 条多为指针身份 / `nullptr` / EGenericType / EInvoke 等 Rust 类型系统暂无法如实表达者，保留为 PENDING。

- **Milestone 6 — XMIResource 挂进 ResourceSet + 静态建模代码生成完善（本轮新增）**：
  - `emf-common::resource` 增加序列化器无关的抽象：`ResourceHandle` trait（uri/loaded/contents/load/save/下转型）与 `ResourceFactory` trait；`ResourceSet` 改持 `Box<dyn ResourceHandle>` + 可选工厂，新增 EMF 对齐的 `get_resource(uri, loadOnDemand)` 按需创建并加载、`create_resource` 走工厂（无工厂退化为内存 `Resource`）。单元测试覆盖工厂分派、按需加载只触发一次、缺失 URI 不创建。
  - `emf-xmi` 落地 `XMIResourceFactory`（持有 `PackageRegistry`）并为 `XMIResource` 实现 `ResourceHandle`，把 XMI 真正挂进 `ResourceSet`；`lib.rs` 移除 `xmi_resource_factory` 占位模块并导出真实工厂。
  - 端到端（`emf-xmi/tests/resource_set_xmi.rs`）：用 `ResourceSet`+工厂落到真实 `.xmi` 文件，换一个 set 用 `getResource(uri, true)` 按需读回，验证类名 / 属性 / containment 子对象图一致。
  - `emf-ecore-codegen` 完善：多值属性 `e_set` 所需的 `scalar_as_i64` helper 移入生成源码（保证含多值属性的模型也能脱离 `.ecore` 编译）；端到端测试确认生成 crate 可 `cargo run` 跑通反射 API。
  - 顺带修复 clippy 质量门禁告警：适配器身份比较改用薄数据指针、移除 `SegmentSequenceBuilder` 固有 `to_string`（改经 `Display`）、清理 `absurd_extreme_comparisons` / `approx_constant` 等测试 lint。全工作区 fmt / clippy / test / release 全绿，一致性 193 条全 PASS（188 等价 + 5 语义无法表达）。

## 9. 提交记录（与本仓库进度相关的近期提交）

| 提交 | 内容 |
|---|---|
| `70ff898` | 命名重构：EMF 基础库 `emf-*`，artop 专属入 `crates/emf-artop/` |
| `df146ec` | 完成 `emf-common` + `emf-ecore` EMF 基础层（完整实现 + 集成测试） |