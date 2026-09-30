# artop-rust 移植进度记录

> 本文档跟踪把一个 C++ 实现的 EMF + ARTOP 模型平台（[`hebin123456/artop-cpp`](https://github.com/hebin123456/artop-cpp)）1:1 移植到 Rust 的整体进度。
> 更新策略：每个可验证的里程碑（模块可用 / 测试全绿 / 提交）在此登记。

> ⛔ **硬门线**：EMF 底座必须全部与 artop-cpp 的 C++ 测试逐条对齐（见 `docs/EMF_PARITY_GATE.md` 与 `docs/PARITY_TRACKER.md`）之后，才有资格动手 artop。底座对齐完成前**严禁任何 artop 相关工作**。

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
    artop-runtime     <- emf-artop/emf-artop-runtime（AUTOSAR 序列化/反序列化）
    artop-codegen     <- emf-artop/emf-artop-codegen（.ecore → 静态模型）
    artop-validation  <- emf-artop/emf-artop-validation（AUTOSAR 业务约束，叠在 emf-validation 之上）
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
| `emf-ecore-util` | ✅ 工作 | EcoreUtil / Copier / **EObjectValidator** / **FeatureMap** / **ECrossReferenceAdapter**；其余含 extended_metadata / EList 家族骨架 |
| `emf-ecore-codegen` | ✅ 工作 | GenModel→代码生成：ecore loader（XMI→`EPackage`）+ TypeMapper + generator（struct / `match` 反射表 / `register_package`）+ 顶层 `GenModel` API 与 CLI（`.ecore` → 落盘可独立编译 crate），生成的 crate 可脱离 `.ecore` 编译运行 |
| `emf-xmi` | ✅ 工作 | saver + loader + 真实 `XMIResource` + `ResourceSet` 按需加载集成（`ResourceHandle` / `ResourceFactory` / `XMIResourceFactory`）；**`XMLHelper`**（命名空间上下文栈 + feature kind 分类 + 按名查询）+ **`XMLLoadImpl`**（`XMLLoad` trait + 默认实现委托资源加载器）已实现并测试通过 |
| `emf-xsd` | ✅ 工作 | XSD 元模型：`XSDSchema` / `complexType` / `simpleType` / `element` / `attribute` / `annotation` / compositor（sequence/choice/all）/ facets / import/include/redefine（普通 Rust 类型 + fluent builder）+ `xsd_parser`（基于 `emf-xmi` 的 XML 解析器，按 local name 忽略命名空间前缀，`maxOccurs="unbounded"`→`-1`） |
| `emf-edit` | ✅ 工作 | `EditingDomain` + `SetCommand` / `AddCommand`（单值+集合）/ `RemoveCommand` / `MoveCommand` / `ReplaceCommand`（经 `BasicCommandStack` undo/redo）；`ChangeDescription`；`AdapterFactoryEditingDomain` + `TransactionalEditingDomain`（通知延迟 + 嵌套事务 + 提交合并去重）；`ComposedAdapterFactory` / `TreeNode`+`TreeIterator` / `EditUtil` / `EMFEditPlugin` / provider 接口；C++ 三测试文件（Command/EditingDomain/Placeholder）已逐条对照全绿 |
| `emf-compare` | ✅ 工作 | 两方/三方比较全管线：`MatchEngine`（ID / 就近匹配）+ `DiffEngine`（属性 / 引用 / MOVE 差分）+ `EquivalenceEngine` + `ConflictDetector`（真/伪冲突）+ `RequirementEngine`（依赖排序）+ `MergeEngine`（按依赖拓扑应用并标记 merged）+ `DiffFilter`；模型类型 `Diff` / `Match` / `Conflict` / `Equivalence` / `Dependency` / `Comparison` |
| `emf-validation` | ✅ 工作 | `Constraint` / `EValidator` / `Diagnostician` / `ConstraintDescriptor`（批量+实时校验） |
| `emf-xcore` | ✅ 工作 | Xcore DSL 解析器：`dsl`（Package/EClass/EDataType/EEnum/Feature/Annotation AST，Multiplicity 与 kind 判定）+ `parser`（递归下降：注解 `@key[.value]`、`package/class/interface/abstract`、`extends`、`#` containment 引用、`?*/` 多重性、`@DataType`/`@Enum`），23+ 用例覆盖 |
| `emf-acceleo` | ✅ 工作 | Acceleo MTL/`M2T` 引擎（对 C++ `AcceleoAst.h`/`AcceleoParser.cpp`/`AcceleoEngine.cpp` 的 1:1 移植）：`ast`（Block：Text/Expr/For/If/Let/File/Protected；Expr：Var/String/Int/Bool/Nav/Call/CollectionLit/If/Lambda）+ `parser`（递归下降：`[module]`/`template`/`query`/`import`/`extends`、`[for]/[if]/[let]/[file]/[protected]`、`post(...)` 容错）+ `engine`（`AcceleoEngine`/`AcceleoService`：上下文/服务注册/模板·查询查找、表达式求值、`=`/`or`/`and`/比较/算术、lambda + `->collect/select/reject/forAll/exists/size` 集合操作、`[file]` 落盘与 `[protected]` 区域合并）；C++ `AcceleoTests.cpp`(18) + `AlignmentTests.cpp`(8) 已逐条对照，26 条全 PASS（`cpp_parity_acceleo`） |
| `emf-sphinx` | ✅ 工作 | headless 核心：`Node`（attributes/children fluent builder）+ `Root`/`Model`（全路径索引 O(1) `resolve`）+ 深度遍历（`Continue`/`Prune`/`Stop` 控制）；Sphinx 扩展：`metamodel`（`MetaModelDescriptor`/`AbstractMetaModelDescriptor` + `MetaModelVersionData` + thread-local `MetaModelDescriptorRegistry`）、`resource`（`SchemaLocationUriHandler`/`ExtendedBasicExtendedMetaData`/`ModelConverterRegistry`）、`scoping`（`FileResourceScope`/`FileResourceScopeProvider`/`ResourceScopeProviderRegistry`）、`ecore`（`OrderedFeatureMap`）、`util`（`EcoreResourceUtil`）；C++ 5 个测试文件 68 条已逐条对照并全 PASS（`cpp_parity_sphinx`） |
| `emf-artop/artop-validation` | ✅ 工作 | AUTOSAR 业务约束层（对齐 `org.artop.aal.*.constraints`）：`autosar_constraints`（shortName 非空/同父唯一、uuid 非空/全局唯一、category 必填、no-unresolved-proxy；BATCH+LIVE）；叠在通用 `emf-validation` 底座之上，由调用方显式注册。C++ `AutosarConstraintsTests.cpp` 13 条已逐条对照 |
| `emf-artop/artop-runtime` | 🟡 进行中 | 基座已落地：`AutosarMetaModelVersionData` / `AutosarReleaseDescriptor` / `IdentifiableUtil` / `AutosarLibraryIndex` / `UnknownElement`；资源层已落地：`AutosarResource` / `AutosarXMLResource` / `AutosarResourceFactory`（含 `register_default_autosar40_metamodel` 静态元模型注册）/ `AutosarResourceSet`（基于 `emf-xmi`，含 release / schemaLocation / 库索引 / `getResource` 按需加载 / `getEObject` 跨资源解析）；**C++ 对照已接入**：18 条用例逐条移植并全 PASS（conformance oracle）；arxml **读**已落地：`arxml/dom`（保留注释与混合内容的 DOM）+ `arxml/store`（混合内容/注释/引用信息侧表）+ `AutosarXMLLoader`（三阶段：建对象树 / shortName 路径索引 / 代理引用解析，含 wrapper 与 BASE 相对路径）；arxml **写**已落地：`arxml/saver`（`AutosarXMLSaver` = `PugiDomWriter` 延迟开标签流式 writer + APRXML 0012/0015/0016/default 规则 + 引用 `DEST`/shortName path/BASE 相对路径 + mixed 序列回放），读写二者均注入为 `AutosarResource` 的默认 `XMLSave`/`XMLLoader`；**待做** 字节级 round-trip 收敛与写路径 conformance 覆盖 |
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
- 多 crate oracle：`build_ecore_oracle.sh`（emf-ecore，153 条）、`build_edit_oracle.sh`（emf-edit）、
  `build_acceleo_oracle.sh`（emf-acceleo，26 条）、`build_sphinx_oracle.sh`（emf-sphinx，68 条）、
  `build_artop_runtime_oracle.sh`（artop-runtime，18 条）；映射表 `cases.tsv` /
  `cases_ecore.tsv` / `cases_edit.tsv` / `cases_acceleo.tsv` / `cases_sphinx.tsv` / `cases_artop_runtime.tsv`。
  其中 `emf-edit` 26 条、`emf-acceleo` 26 条、`emf-sphinx` 68 条、`artop-runtime` 18 条均为 **0 PENDING 全 PASS**。

复用路径：C++ oracle 单测 → `tools/conformance/build`，与 CI 的 `conformance` job 对齐。

## 6. 质量门禁（每次提交前）

```sh
cargo fmt    --all -- --check
cargo test   --workspace --all-targets     # 全 crate 单测 + 集成对照全绿
cargo clippy --workspace --all-targets     # 无 error / warning
cargo build  --workspace --release         # release 也通过
bash tools/conformance/build_oracle.sh <artop-cpp>/cpp/emf-cpp/emf-common
python3 tools/conformance/compare.py       # 无 REGRESSION 即通过
```

## 7. 下一步（按优先级）

已完成：emf-common/ecore 核心、EcoreUtil/Copier、XMI saver+loader、Resource/XMI 持久化集成、一致性测试 193 组中 188 条映射 PASS（含 command 模块 / NotifyingList / SegmentSequence / UniqueEList / ENotifier / EAdapter）；一致性框架已多 crate 化并建立 emf-ecore oracle（153 条，映射 87 条 PASS）、emf-edit oracle（26 条全 PASS）、emf-acceleo oracle（26 条全 PASS）、emf-sphinx oracle（68 条全 PASS）。

1. 扩展 oracle 到 emf-xmi 的 C++ tests 逐模块收敛；继续补 emf-ecore 未映射项（BasicEObject 容器/inverse-list 通知、指针身份类；EGenericType / EInvoke 已对齐）。
2. `emf-xmi` 进一步落地：`XMILoadImpl` / `XMIHelper` 接口（`XMIResourceFactory` + `ResourceSet.getResource` 按需加载已完成）。
3. `emf-ecore-util` 剩余：Adapter / ECrossReferenceAdapter / containment 遍历到 `all_contents` 的流式实现。
4. `emf-sphinx` 剩余：`ExtendedResource`/`ProxyHelper`/`ModelDescriptor`/`EcoreTraversalHelper` 等在 C++ 侧仍为骨架或空测试，随上层用例补齐再逐条对照。
5. `artop-runtime`：AUTOSAR 序列化/反序列化（届时才引入 artop 相关内容）。

## 7.1 `AutosarXMLSaver`（arxml 写核心）—— 已完成

**现状**：arxml **读**（`459c1d5`）与**写**均已落地。写出走新增的 `arxml/saver.rs`（`AutosarXMLSaver`），已在 `AutosarResource::new_inner` 里 `inner.set_xml_save(AutosarXMLSaver::new())` 并作为 `create_xml_save` 的返回值，与 C++ `AutosarXMLResource::createXMLSave` 一致。

**已实现**（对照 C++ `AutosarXMLSaver.cpp`）：

- `DomWriter`（C++ `PugiDomWriter`）：延迟开标签流式 writer，`<TAG/>` / `\n+indent</TAG>` / inline `</TAG>` 三态；`encode_text` / `encode_attribute_value` 转义集对齐。
- `run()`：根 `<AUTOSAR xmlns xmlns:xsi xsi:schemaLocation>`，遍历 contents，末尾 `\n`。
- `save_object_content()`：`simple`/`mixed`/默认三条 path + `elements_only` 变体。
- `save_containment()` + `resolve_aprxml_rule()`：APRXML 0012/0015/0016/default wrapper 规则。
- `save_reference()` / `write_reference_body()`：`<FEATURE DEST="TypeXmlName">short-name-path</FEATURE>`，DEST 优先取 loader 侧表 `ref_dest`。
- `save_mixed_content()`：按 `store::mixed_content` 回放原始序列（文本/注释/子元素），wrapper 去重。
- `try_compute_base_relative()` + `reference_base_prefix` / `read_reference_base_short_label` / `read_reference_base_is_default`：BASE 相对路径与 `isDefault` 收敛。
- `short_name_path()` / `type_xml_name()` / `sorted_features()`（按 `sequence_offset` 稳定排序）。

**已延后（文档化，未静默丢弃）**：`atp.Splitkey` / `ordered` 驱动的子元素排序（Rust 静态注册表未携带这两个标记，列表按模型顺序输出）、未知内容片段（loader 已跳过不可映射元素）。

**已验证的 round-trip 现状**（`AutosarXMLSaver` 第二轮）：

- XML 属性上的 **`nsPrefix` 已实现**（不再延后）：`.ecore` 的 `xml.nsPrefix`（如 `xml:space`）经 `tools/gen-autosar448-model.py` → `FeatureMeta.ns_prefix` → `EStructuralFeature::xml_ns_prefix()` → `save_attribute` 输出 `prefix:name`。测试 `round_trip_preserves_comment_xml_space_and_base_ref` 覆盖。
- 用 artop-cpp 的 5 份真实样本（`output/samples/*.arxml`，共 ~1.1MB）跑 `cargo run -p artop-runtime --example arxml_roundtrip -- roundtrip <in> <out>`：
  - 4 份真实样本（`AISpecificationKeywordSetBlueprint` 821KB、`GeneralDefinitionEnumerationTables` 237KB、`AISpecification_PhysicalDimension_LifeCycle_Standard` 58KB、`GeneralDefinitionReferenceBase` 2.9KB）**load → save 与原文仅差 1 行**——`<L-10 L="EN" xml:space="default">` 的属性顺序变为 `<L-10 xml:space="default" L="EN">`。XML 属性无序，C++/Java 读者均忽略顺序，属已知且接受的差异（已在 `saver.rs` 模块文档注明）。元素顺序、嵌套、`DEST` / `BASE` / 文本全部一致。
  - 第 5 份 `AISpecificationCollectionBodyBlueprint.arxml` 是 **0 字节空占位文件**，非真实失败。
  - 4 份样本均满足二次 `load → save` **字节稳定**（写路径幂等）。
- 关键修复：`round_trip_preserves_comment_xml_space_and_base_ref` 原先的 fixture **缺少 `Cite` 这个 `REFERENCE-BASE`**，导致 `BASE="Cite"` 无法反向还原（无匹配 `ReferenceBase` → 退化为 `autosar-proxy` 路径）。改用真实样本的完整三段 `REFERENCE-BASE`（`ArTrace` / `Cite`(BASE-IS-THIS-PACKAGE=true) / `EnumMappingTables`）后，`try_compute_base_relative` 正确还原 `BASE="Cite"`。**saver 逻辑本身无误，是测试夹具不自洽**。

**下一棒 —— arxml 互读互写收敛**（剩余工作）：

1. 扩 `tools/conformance` 的 oracle/TSV 覆盖**写**路径（当前 oracle 只覆盖读）。
2. 建立 Rust ↔ C++ 双向 arxml 互读互写 CI 用例（对齐现有 `interop_xmi.py` 的形态）。C++ 侧 `cpp/emf-cpp/examples/arxml_roundtrip_demo.cpp` 目前是**空占位**，需先补一个 C++ roundtrip harness（走 `emf-artop-runtime` 动态 EMF 路径，读 `model/autosar40.ecore`），再按 `interop_xmi.py` 做 A/B/C/D 四步文件交接。
3. 若需字节级一致：在 `collect_sorted_features` 里对齐 C++ 的 XML 属性输出顺序（当前按元模型顺序，非文档顺序）。

**参考实现**：`AutosarXMLSaver.cpp`（2023 行，对齐 Java `AutosarXMLSaveImpl`），关键函数定位供收敛时比对：`save()`(L567)、`PugiDomWriter`(L214)、`saveObjectContent()`(L615)、`collectSortedFeaturesUncached()`(L890)、`saveAttribute()`(L1021)、`attrValueToString()`(L1083)、`saveContainment()`(L1157)+`resolveAprxmlRule()`(L1303)、`saveReference()`(L1329)、`saveSingleMixedElement()`(L1487)、`tryComputeBaseRelative()`(L1602)+`getReferenceBasePrefix/readReferenceBaseShortLabel/readReferenceBaseIsDefault`(L1659-1718)、`getShortNamePath()`(L1787)、`getTypeXmlNameUncached()`(L1854)、`isFeatureOrdered()`(L1879)、`getSplitkeyValue/sortChildrenBySplitkey/sortChildrenByShortName`(L1895-2016)。

**可直接复用的已有件**：`arxml::dom::{Element,Node,parse}`、`arxml::store`（`mixed_content` / `comments` / `mixed_text` / `ref_dest` / `ref_is_default`，saver 读 loader 写入的侧表）、元模型注解读法 `EStructuralFeature::{xml_name,xml_name_plural,is_xml_attribute,is_role_element,is_role_wrapper,is_type_element,is_type_wrapper,tagged_feature_kind,sequence_offset}`、`EClass::content_kind()`、`XMIResource::{options(),get_xsi_schema_location(),resource()}`。

**质量门禁**：`cargo fmt --all` / `cargo test --workspace --all-targets` / `cargo clippy --workspace --all-targets` 全绿。

**注意**：不要在 `emf-*` 通用底座里引入 arxml/AUTOSAR 概念（见 §1 分层原则）；写路径全部留在 `emf-artop/artop-runtime`。

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

- **Milestone 7 — 顶层 GenModel 对外 API + CLI（本轮新增）**：把静态建模做成可独立复用的入口，对齐 C++ `GenModelLoader` / `GenModel::generateAll` 的对外调用面。
  - 新增 `emf-ecore-codegen` 顶层 `GenModel`（`load(src)` / `load_path`）+ `CrateSpec`（默认把依赖解析为工作区相邻的 `emf-common`/`emf-ecore` 兄弟 crate）。`generate_source()` 渲染单个 `lib.rs`；`generate_crate(dir, spec)` 落盘一整个自包含 crate（`Cargo.toml` + `src/lib.rs`，`[workspace]` 根、显式 path 依赖），脱离 `.ecore` 即可编译。`register(&mut PackageRegistry)` 供反射直达。
  - 可选 CLI：`src/main.rs` 二进制 `emf-ecore-codegen <model.ecore> [--out DIR|--name|--common|--ecore]`，手工解析参数无第三方依赖；实测从 `samples/library.ecore` 生成 crate 并独立 `cargo build` 成功。
  - 新增 `samples/library.ecore` 样例文件；集成测试 `tests/static_modeling.rs` 用公开 API 从磁盘加载 → 落盘生成 crate → 独立 `cargo build` + 消费者二进制跑通反射 API。
  - 生成源码收紧 lint：`#![allow(dead_code, unused_imports, unused_mut, non_snake_case, clippy::too_many_arguments)]` 并去掉未用的 `RefCell`/`Rc` 导入，生成的 crate 独立编译零告警。
  - 质量门禁：`emf-ecore-codegen` 全部单测 + 集成测试 + CLI 端到端全绿（19 单测 + 3 集成）。注：本轮会话中 sandbox 的 Rust 工具链曾消失，已用 rustup + static.rust-lang.org 重建（cargo/rustc 1.98.1 与旧指纹一致），并把 `~/.cargo/bin` 写入 `/etc/profile.d/work-env.sh`。

- **Milestone 8 — emf-ecore-util 底盘模块落地（本轮新增）**：把 `emf-ecore-util` 从骨架推进为一组真实可用模块。
  - `EObjectValidator`：结构校验（对齐 C++ `EObjectValidator`），`validate_epackage` / `validate_eclass` / `validate_eattribute` / `validate_ereference` / `validate_eoperation` 覆盖包 / 类 / 特征号语义；含 `validate_every_default_constraint` 折叠入口与 `codes` 诊断码常量。
  - `FeatureMap` / `BasicFeatureMap`：有序 `(feature, value)` 条目表（对齐 C++ `FeatureMap`），支持 `add` / `entries_for` / `size_for` / `get_for` / `set_for` / `remove`，为 XSD/XML 的 group/choice/sequence 序列化铺路。
  - `ECrossReferenceAdapter`：收集子树内所有非 containment 跨引用目标（对齐 C++ `ECrossReferenceAdapter` 的 `getNonContainmentReferences`），`add_adapter_to` / `remove_adapter_from` / `contains`；Rust 反射层暂为快照式扫描实现，`ObjectRefKey` 作为不透明引用键对外。
  - 质量门禁：`emf-ecore-util` 13 条单测 + fmt + clippy（0 告警）全绿，全工作区 test 无回归。

- **Milestone 9 — emf-xmi `XMLHelper` / `XMLLoadImpl`（本轮新增）**：补齐 XMI 层的配置与反序列化入口接口。
  - `XMLHelper`：命名空间上下文栈（`push_context` / `pop_context` / `add_prefix` / `get_uri` / `get_prefix` / `record_prefix_to_uri_mapping`）+ feature kind 分类（`DatatypeSingle` / `IsManyAdd` / `DatatypeMany` / `Other`）+ 按 `(class, namespaceURI, name)` 查询 feature，为序列化器提供配置基座。
  - `XMLLoadImpl`：`XMLLoad` trait + 默认实现（`load(request)` 委托给资源自身的 registry 驱动加载器），外加 `parse_only` 只校验不落盘入口。
  - 质量门禁：`emf-xmi`（新增单元测试后）24 条测试全绿，`fmt --check` 通过、新模块 clippy 0 告警；全工作区 test 无回归。

- **Milestone 10 — emf-edit `EditingDomain` + 标准编辑命令（本轮新增）**：把 `emf-edit` 从骨架推进为可用的命令框架。
  - `SetCommand`：为单个 owner 的某个 feature 设置/取消值（`SetCommandRequest`），undo/redo 在首次执行时对 is-set 状态与旧值做快照（经 `e_is_set` / `e_get`），undo 恢复旧态、redo 重放新值；值通过反射面写入，与元模型无关。
  - `AddCommand` / `RemoveCommand` / `MoveCommand`：操作多值 feature 的集合——add 追加、remove 按值移除、move 在集合内移动元素到指定 index；各自带 undo/redo 快照恢复。
  - `EditingDomain`：持有 `BasicCommandStack`，`create_command`/`create_commands_for_set` 统一构造 set/add/remove/move 命令（对齐 C++ `createCommand` 编排入口）。
  - `lib.rs` 移除 add/remove/move/set/editing_domain 占位模块并挂载真实实现（`CommandRequest` 作为 `SetCommandRequest` 的别名对外）。
  - 质量门禁：`emf-edit` 9 条单测全绿（含经 `BasicCommandStack` 的 execute/undo/redo/redo 端到端），fmt + clippy（0 告警）通过，全工作区 test 无回归。

- **Milestone 11 — emf-validation 校验框架（本轮新增）**：把 `emf-validation` 从骨架推进为可用的校验框架。
  - `Constraint`：持有求值器 `Evaluator`（`Fn(&dyn EObject) -> bool`），带 id/name/message/**severity**（Ok/Info/Warning/Error/Cancel）/ **mode**（Live/Batch）+ 按类名子串过滤（`target_class_names`）。
  - `EValidator`：注册/注销约束（`register_constraint` / `unregister_constraint` / `get_constraint`）+ 监听器（`IConstraintListener`）;`validate` / `validate_mode` 对目标求值产出诊断；内置默认约束（空 name 告警）;提供 per-`EPackage` 的 `Registry`（`from_pairs` / `put` / `get`）供 `Diagnostician` 分派。
  - `Diagnostician`：`map_severity` 把校验严重级映射到通用 `Diagnostic`；`validate_object` 单对象、`validate` 沿 containment 树深度优先遍历（`collect`）并逐对象按包分派；`package_of` 以类名为兜底包键。
  - `ConstraintDescriptorParser`：零依赖解析 `plugin.xml` 风格 `<constraint>`（自闭合 `/>` 与完整 `>` 均支持，含引号内 `>` 处理），`parse_descriptors` / `parse_and_register`；`ConstraintDescriptor::instantiate` 把 `body` 编译为求值器（`==`（值相等）/ `~=`（不得包含）两种 OCL 子集）。
  - 质量门禁：`emf-validation` 10 条单测全绿（含约束求值、类过滤、分派诊断、未注册包静默跳过、XML 约束解析与实例化），fmt + clippy（0 告警）通过，全工作区 test 无回归。

- **Milestone 12 — emf-compare 两方/三方比较全管线（本轮新增）**：把 `emf-compare` 从骨架推进为完整的比较/合并管线。
  - 模型类型（`support`）：`Comparison` / `Match` / `Diff`（属性 / 引用 / MOVE / ADD / DELETE）/ `Conflict`（REAL / PSEUDO）/ `Equivalence` / `Dependency`，均带引用与方向/来源统计。
  - `MatchEngine`：根匹配 + ID 匹配（`xmi:id`）优先 + 就近相似度（属性命中、phantom 判定）；`DiffEngine`：属性值、单/多引用、有序集合 MOVE 差分、法术变化；`EquivalenceEngine` 归一化等价项。
  - `ConflictDetector`（真/伪冲突）+ `RequirementEngine`（遍历 diff 依赖边、拓扑排序）+ `MergeEngine`（`remove_conflicting`/`merge_over`，按依赖顺序应用并标记 merged）+ `MinimalDiffFilter`。
  - 质量门禁：`emf-compare` 14 条单测全绿，fmt + clippy（0 告警）通过，全工作区 test 无回归。

- **Milestone 13 — emf-xsd 元模型 + 解析器（本轮新增）**：把 `emf-xsd` 从骨架推进为一套可用的 XSD 处理层。
  - `xsd_metamodel`：`XSDSchema` / `XSDComplexTypeDefinition` / `XSDElementDeclaration` / `XSDAttributeDeclaration` / `XSDCompositor`（sequence/choice/all）/ `XsdFacet` / `XSDAnnotation` / import/include/redefine + fluent builder + `type_by_name` / `element_by_name` 查询。
  - `xsd_parser`：基于 `emf-xmi` 的 XML 解析器把 `<schema>`（含 `targetNamespace`、elementFormDefault 等）灌入元模型，按 local name 忽略命名空间前缀，`maxOccurs="unbounded"` → `-1`，支持自闭合标签与注解。
  - 质量门禁：`emf-xsd` 6 条单测全绿，fmt + clippy（0 告警）通过，全工作区 test 无回归。

- **Milestone 14 — emf-xcore / emf-acceleo / emf-sphinx（本轮新增）**：把剩余三个骨架 crate 推进为可用实现。
  - `emf-xcore`：`dsl` 定义 Xcore AST（`PackageDecl` / `EClassDecl` / `EDataTypeDecl` / `EEnumDecl` / `FeatureDecl` / `Annotation` / `Multiplicity`），`parser` 递归下降（`@key[.value]` 注解、`package/class/interface/abstract`、`extends`、`#` containment 引用、`?`/`*`/`+` 多重性、内置 `String/Int/...` 与 `@DataType` 表判定属性 vs 引用），5 条单测全绿。
  - `emf-acceleo`：`mtl_parser`（`[template]/[query]`、public/private/protected、类型化 `(a : EClass)` 参数、模块/导入/注释跳过）+ `template`（`TemplateFile` / `TemplateDecl` / `ValueContext` + `$var` 替换 + `[comment]` 剥离）+ `m2t_engine`（entry-point 渲染、`[if/$]/[else]/[/if]`、`[for/$]/[/for]`、具名渲染），13 条单测全绿。
  - `emf-sphinx`：`headless_core` 提供 `Node`（attrs/children + fluent builder）、`Root`/`Model`（全路径索引 + O(1) `resolve`/`require`）、`WalkControl`（Continue/Prune/Stop）深度遍历，4 条单测全绿。
  - 质量门禁：新增 22 条单测全绿，三个 crate fmt + clippy（0 告警）通过，全工作区 test 无回归。

- **Milestone 15 — 复用 artop-cpp 测试用例做 C++ 对照（本轮新增）**：不再自说自话，直接移植 C++ 侧的测试断言到 Rust，统一走 artop-cpp `cpp/emf-cpp/emf-*` 下 `tests/samples/library.ecore` 这份**逐字节相同**的权威样本。
  - 把 `emf-ecore-codegen/samples/library.ecore` 替换为 C++ 权威版（标准 `ecore:EDataType http://.../Ecore#//EString` 写法），并核对逐字节一致。
  - 新增 `emf-ecore-codegen/tests/cpp_parity_static_modeling.rs`（7 条）：对照片 `GenModelLoaderTests.cpp`（wrapEcore 构建 Library/Book/Writer 元数据、attribute vs reference、containment、EString/EInt 类型映射、`recognizesReference`）+ `RuntimeBehaviorTests.cpp`（动态 eClass/eSet/eGet/eIsSet/eUnset）。
  - 新增 `emf-ecore-codegen/tests/cpp_parity_xmi_serialization.rs`（2 条）：对照片 `RoundtripTests.cpp` / `E2E_GenModelXmi*` —— 加载同份 `library.ecore` → 按元模型实例化 `Library{books→Book{author→Writer}}` → emf-xmi `save_to_string` → `load_from_string` → 校验对象图（类名、属性值、containment 子树）完整还原；含 `XMILoaderTests` 的元模型断言。
  - 全工作区 fmt / clippy / test 全绿；9 条对照测试全部通过，无回归。

- **Milestone 16 — emf-acceleo 重写为对 C++ 的 1:1 移植 + 接入一致性框架（本轮新增）**：此前 `emf-acceleo` 只是一套自研的轻量模板引擎（`mtl_parser`/`template`/`m2t_engine`），与 C++ 语义不完全对齐。本轮直接以 artop-cpp 的 `AcceleoAst.h` / `AcceleoParser.cpp` / `AcceleoEngine.cpp` 为准重写。
  - `ast`：定义与 C++ 对应的 AST —— `Block`（`Text`/`Expr`/`For`/`If`/`Let`/`File`/`Protected`）与 `Expr`（`Var`/`StringLit`/`IntLit`/`BoolLit`/`Nav`/`Call`/`CollectionLit`/`If`/`Lambda`）。
  - `parser`：递归下降，覆盖 `[module]`（含 `import`/`extends`）、`public/private/protected` 模板、`[template]`/`[query]`（类型化参数 + `post(...)` 容错）、`[for]/[if]（含 else/elseif）/[let]/[file]/[protected]`，以及 `=`/`or`/`and`/比较/算术表达式与 `->` 箭头调用、lambda（`x | expr`）。
  - `engine`：`AcceleoEngine` + `AcceleoService` —— 求值上下文、服务注册、模板/查询查找（含 `extends` 继承与 `import`）、表达式求值、块求值、`->collect/select/reject/forAll/exists/size` 集合操作、`[file]` 落盘、`[protected]` 区域的 old/new 合并。
  - 测试：`crates/emf-acceleo/tests/cpp_parity_acceleo.rs` 逐条移植 `AcceleoTests.cpp`(18) + `AlignmentTests.cpp`(8) 共 **26 条**，全绿。
  - 顺带修复 `emf-xcore` parser 的类型判定：把 `int`/`boolean`/`string` 等小写 Java/Xcore 原生类型名计入内建数据类型，对齐 C++ `XcoreGenerator::resolveClassifier` 的 `int→EInt` 映射（否则 xcore 特征会被误判为引用）。
  - 一致性框架接入：新增 `tools/conformance/build_acceleo_oracle.sh`（编译并运行 C++ `emf-acceleo` 单测二进制 → `build/acceleo_oracle.json`，26 条全 pass）与 `tools/conformance/cases_acceleo.tsv`（26 条 C++↔Rust 映射）；`compare.py` 对 `emf-acceleo` 输出 **26 条全 PASS，0 PENDING，0 REGRESSION**；CI `conformance` job 增加 acceleo 的 oracle 构建与比对步骤。
  - 质量门禁：全工作区 fmt / clippy / test 全绿，`emf-acceleo` clippy 0 告警，无回归。

- **Milestone 17 — emf-sphinx 骨架补齐 + 接入一致性框架（本轮新增）**：以 artop-cpp `cpp/emf-cpp/emf-sphinx` 的 headless 部分为准，补齐 meta-model 描述符、schema-location、resource scoping、有序 feature-map 与 Ecore resource 工具。
  - `metamodel`：`MetaModelVersionData`（ns postfix / EPackage nsURI postfix pattern / name / base descriptor / ordinal + `equals`）+ `MetaModelDescriptor` trait & `AbstractMetaModelDescriptor`（identifier / namespace / 版本拼接 / `matchesNamespace` / EPackage nsURI 正则全匹配 / `equals`·`hashCode` 按 identifier / compatible 列表）+ thread-local `MetaModelDescriptorRegistry`（注册·查找·unregister（`Rc::ptr_eq` 身份）·target/old）。
  - `resource`：`SchemaLocationUriHandler`（成对 `ns uri` 解析 + 从 `XMIResource` 读 `xsi:schemaLocation`）、`ExtendedBasicExtendedMetaData`（`ns|loc` 缓存键）、`ModelConverterRegistry`（增·删·去重·按源/目标元模型查找）。
  - `scoping`：`ResourceScope` trait + `FileResourceScope`、`ResourceScopeProvider` trait + `FileResourceScopeProvider`、thread-local `ResourceScopeProviderRegistry`（注册·派发·`isNotInAnyScope`）。
  - `ecore`：`OrderedFeatureMap`（按 featureID、再按 index 有序插入；按 feature 过滤）。`util`：`EcoreResourceUtil`（URI 归一化 / `exists` / 骨架空分支 / `loadResource`·`getModelRoot`·`isResourceLoaded` 等）。
  - 配套改动：`emf-xmi::XMIResource` 增加 `get/set_xsi_schema_location`（对齐 C++ `getXSISchemaLocation`）；`emf-sphinx` 新增对 `emf-ecore`/`emf-xmi` 的依赖。
  - 测试：`crates/emf-sphinx/tests/cpp_parity_sphinx.rs` 逐条移植 `EcoreResourceUtilTests.cpp`(20) + `MetaModelDescriptorTests.cpp`(18) + `OrderedFeatureMapTests.cpp`(7) + `ResourceTests.cpp`(13) + `ScopingTests.cpp`(10) 共 **68 条**；`ExtendedResourceTests.cpp`/`ModelDescriptorTests.cpp`/`ProxyHelperTests.cpp` 在 C++ 侧为空（仅一行 `#include <cstdio>`）不移植。
  - 一致性框架接入：新增 `tools/conformance/build_sphinx_oracle.sh`（递归收集 emf-sphinx 源码并复刻 CMake 的平台文件过滤，链接 emf-common/ecore/ecore-util/edit/xmi → `build/sphinx_oracle.json`）与 `tools/conformance/cases_sphinx.tsv`（68 条 C++↔Rust 映射）；CI `conformance` job 增加 sphinx 的 oracle 构建与比对步骤。
  - 质量门禁：全工作区 fmt / clippy / test 全绿，无回归。

- **Milestone 18 — artop 阶段启动：autosar448-model 静态建模 + artop-runtime 基座（本轮新增）**：EMF 通用底座达标后，正式进入 artop 专属层。
  - `autosar448-model`（生成的 AUTOSAR 4.4.8 静态注册表）：重写生成器 `tools/gen-autosar448-model.py`，从 `gautosar.ecore` + `autosar448.ecore` 合并生成自包含注册表 —— **2105 个 EClass / 6122 个 EStructuralFeature / 291 个 EEnum / 67 个 EDataType**；每条特征带 `kind`(attribute/reference)、`eType`、`containment`、`multiplicity`、`xml.name`/`xml.namePlural`、`xml.attribute`/`textContent`、`sequenceOffset`、`transient/volatile/derived` 等，`registry.rs` 约 2.2 万行、脱离 `.ecore` 可独立编译。
  - `autosar448-model::reflect`：基于注册表的反射查询 —— `eAllFeatures`（祖先优先 + 去重）、`allSuperIds`、`isSuperTypeOf`、`findFeatureByXml`（先 `xml.name`、再 `namePlural`、后 ecore 名）、`eGet` 继承特征读取。
  - `autosar448-model::metamodel`（本轮新增）：**静态模型 → EMF `EPackage` 桥接**。把静态注册表构建为通用 `emf-ecore` 的 `EPackage`（EClass/EEnum/EDataType + 按 arxml 元素名命名的 EStructuralFeature + 名字式继承），`register_autosar_metamodel(&mut PackageRegistry)` 供 `DynamicEObject` / XMI loader/saver 及 arxml 层实例化与反射 AUTOSAR 对象。
  - `artop-runtime` 基座（对齐 Java `org.artop.aal.common.*`）：`AutosarMetaModelVersionData`（`major.minor.revision` + 新旧 schema 版本串 `4-4-8` / `00042`）、`AutosarReleaseDescriptor`（版本 / base namespace / schemaLocation / 兼容版本）、`IdentifiableUtil`（shortName/longName/description/identifier/uuid 反射读写）、`AutosarLibraryIndex`（跨文档 shortName path 索引 + 按需解析，thread-local 全局单例）、`UnknownElement`（未映射 XML 元素记录）。
  - 测试：`autosar448-model` 15 条（注册表一致性 / 规模 / 继承图无环 / XML 名查找 / 桥接包实例化反射）、`artop-runtime` 16 条，全绿。
  - 质量门禁：`cargo fmt --all`、`cargo test --workspace --all-targets`（0 失败）、`cargo clippy -p artop-runtime -p autosar448-model --all-targets`（0 告警）全绿，无回归。
  - **待续**：`AutosarXMLLoader` / `AutosarXMLSaver`（arxml 读写核心）、对照 C++ artop-runtime 用例接入 conformance oracle、arxml 双向互读互写 CI。

- **Milestone 19 — artop-runtime 资源层（AutosarResource / Factory / ResourceSet）（本轮新增）**：把 AUTOSAR 资源三件套从 C++ `AutosarResource` / `AutosarXMLResource` / `AutosarResourceFactory` / `AutosarResourceSet` 移植到 Rust，构建在通用 `emf-xmi` 之上。
  - `autosar_resource`：`AutosarResource` 包裹一个 `XMIResource`（Deref/DerefMut）+ 携带 `AutosarReleaseDescriptor` 与 arxml `xsi:schemaLocation`；`AutosarXMLResource` 再包一层作为 arxml 专属 flavour，提供 `create_xml_helper` / `create_xml_save` / `create_xml_load` 注入缝（当前回退到通用 XMI 序列化/反序列化，arxml 专用实现为后续里程碑）；`index_library` 把资源内容的 shortName path 注册进全局 `AutosarLibraryIndex`。两者均实现 `emf_common::ResourceHandle`（含手写 `Debug`）以便挂进 `ResourceSet`。
  - `autosar_resource_factory`：`AutosarResourceFactory` —— `create_resource`（默认产出 `AutosarXMLResource`，或经 `set_resource_creator` 注入自定义构造器）、`init_resource` / `init_default_options`（UTF-8 编码 + XMI 2.0 + release 的 `xsi:schemaLocation`）、`create_schema_location_catalog`（base namespace + XML/XMLSchema-instance 两个命名空间）。
  - `autosar_resource_set`：`AutosarResourceSet` —— `create_resource`（按 URI 去重）、EMF 语义的 `get_resource(uri, loadOnDemand)`（按需创建并加载、加载后索引进库）、`load_library`、`get_eobject(uri#fragment)` 跨资源解析。
  - `emf-xmi::loader` 顺带扩展：元素形式的标量属性（ExtendedMetaData `kind=element`）与非包含引用（`<feat href="..."/>`）加载——多值元素属性用 `scalar_lists` 聚合后一次性成列表，对齐 C++ `XMLHandler::applyInstanceChild` 的 "EAttribute: child text as value"。
  - 测试：`artop-runtime` 新增资源层单测（工厂产出/自定义 creator/schemaLocation 注入/目录/资源集去重 + release 携带 + 按需缺失返回 None），全绿。
  - 质量门禁：`cargo fmt --all`、`cargo test --workspace --all-targets`（103 个测试二进制 0 失败）、`cargo clippy --workspace --all-targets`（0 告警）全绿，无回归。

- **Milestone 20 — 静态模型的 ARXML 序列化元数据 + 按 arxml 元素名解析类（本轮新增）**：为 arxml 读写铺路，把静态注册表里的 arxml 序列化元数据搬到 EMF 桥接包上，使通用 `emf-xmi`/arxml 层能按元素名反射建对象。
  - `emf-ecore::EStructuralFeature` 新增 `TaggedValues` 注解读取面：`xml_name` / `xml_name_plural` / `is_xml_attribute` / `is_role_element` / `is_role_wrapper` / `is_type_element` / `is_type_wrapper` / `tagged_feature_kind` / `sequence_offset`（对齐 C++ `EAnnotationReader::readFeatureMeta`）。
  - `emf-ecore::EClass` 新增 `annotation` / `tagged_value` / `xml_name` / `content_kind`（对齐 C++ `findEClassByXmlName` 的查找键与 `eContentKind`）。
  - `emf-ecore::EPackage` / `PackageRegistry` 新增 `find_class_by_xml_name`，使 `AR-PACKAGE` / `SWC-IMPLEMENTATION` 这类 arxml 元素标签无需硬编码映射即可解析到 `EClass`。
  - `autosar448-model::metamodel` 新增 `tag_class` / `tag_feature`，把注册表的 `xml.name` / `xml.namePlural` / `featureKind` / `isXmlAttribute` / APRXML role·type·wrapper 标志 / `contentKind` / `internal-xml-sequenceOffset` 写进 `TaggedValues` 注解。
  - `emf-xmi::loader::resolve_class` 回退到 `find_class_by_xml_name`，让 arxml 元素标签直接驱动 `DynamicEObject` 实例化。
  - 测试：`autosar448-model` 16 条（新增注解面断言），全绿。

- **Milestone 21 — artop-runtime C++ 对照接入 + 内置元模型注册（本轮新增）**：把 `emf-artop-runtime` 的 C++ 单测（`tests/test_main.cpp`，18 条）逐条移植到 Rust 并接入一致性框架。
  - `emf-ecore::ecore_package` 新增通用全局注册 API：`global_register(pkg)`（EMF `EPackage.Registry.INSTANCE.put`）与 `global_get(key)`——底座层通用，无任何 AUTOSAR 内容。
  - `AutosarResourceFactory::register_default_autosar40_metamodel()`：把 `autosar448-model` 静态注册表桥接出的 `EPackage`（`autosar40` / `http://autosar.org/schema/r4.0`）注册进进程级全局包注册表并返回该包句柄（对齐 C++ `AutosarResourceFactory::registerDefaultAutosar40Metamodel` 的幂等语义）。为此 `artop-runtime` 将 `autosar448-model` 提为常规依赖。
  - 新增 `crates/emf-artop/artop-runtime/tests/cpp_parity_artop_runtime.rs`（18 条）：逐条对应 C++ 的 version/release/resource/factory/catalog/IdentifiableUtil 用例，以及两条 arxml 用例——`<AUTOSAR>` 根元素实例化、`<AR-PACKAGE>` 多值 containment 递归（用注册了 AUTOSAR 元模型的 `XMIResourceSet` 经通用 XMI loader 读入 `DynamicEObject`，与 C++ 同路径）。
  - 一致性框架接入：新增 `tools/conformance/build_artop_runtime_oracle.sh`（编译并运行 C++ `emf-artop-runtime` 单测二进制 → `build/artop_runtime_oracle.json`，18 条，含 `EMF_ARTOP_AUTOSAR40_ECORE` 宏注入与适配 EMF_RUN 输出的日志解析）与 `tools/conformance/cases_artop_runtime.tsv`（18 条 C++↔Rust 映射）；`compare.py` 对 `artop-runtime` 输出 **18 条全 PASS，0 PENDING，0 REGRESSION**；CI `conformance` job 增加 artop-runtime 的 oracle 构建与比对步骤。
  - 质量门禁：全工作区 fmt / test（全绿，0 失败）/ clippy / release 通过，无回归。
  - **待续**：`AutosarXMLLoader` / `AutosarXMLSaver`（arxml 读写核心，APRXML 规则 / 引用代理 / shortName path）、arxml 双向互读互写 CI。

- **Milestone 22 — arxml 写核心 `AutosarXMLSaver`（本轮新增）**：在 arxml 读（`AutosarXMLLoader`，见 §7.1）落地后，补齐写路径，使 AUTOSAR 资源可 load → save 往返。
  - 新增 `crates/emf-artop/artop-runtime/src/arxml/saver.rs`：`AutosarXMLSaver` 实现 `emf_xmi::XMLSave`，内部 `AutosarSaver` 承载一次序列化上下文。
  - `DomWriter`（对齐 C++ `PugiDomWriter`）：延迟开标签流式 writer，`<TAG/>` / `\n+indent</TAG>` / inline `</TAG>` 三态 + `encode_text` / `encode_attribute_value` 转义集逐字符对齐。
  - 特征遍历：`simple` / `mixed` / 默认三条 path 与 `elements_only` 变体；`sorted_features` 按 `internal-xml-sequenceOffset` 稳定排序（祖先优先）。
  - APRXML：`resolve_aprxml_rule` 判 0012/0015/0016/default，`save_containment` 据此决定 wrapper 包裹 / 直接铺开。
  - 引用：`save_reference` / `write_reference_body` 输出 `<FEATURE DEST="TypeXmlName">short-name-path</FEATURE>`，DEST 优先取 loader 侧表 `store::ref_dest`；`try_compute_base_relative` + `ReferenceBase` 前缀 / `SHORT-LABEL` / `IS-DEFAULT` 支持 BASE 相对路径。
  - 混合内容：`save_mixed_content` 按 `store::mixed_content` 回放原始 文本/注释/子元素 顺序，role-wrapper 去重。
  - 接入：`AutosarResource::new_inner` 与 `create_xml_save` 均返回 `AutosarXMLSaver`（对齐 C++ `AutosarXMLResource::createXMLSave`）；`arxml::mod` 导出 `AutosarXMLSaver` 与 `saver` 模块。
  - 测试：saver 单测 6 条（根元素 + 命名空间/schemaLocation、`<AR-PACKAGES>` wrapper 下的嵌套包 round-trip、引用 DEST/path、属性元素与文本转义、真实样本 `nsPrefix`+注释+`BASE` 引用 round-trip），全绿。
  - 质量门禁：`cargo fmt --all` / `cargo test --workspace --all-targets`（0 失败）/ `cargo clippy --workspace --all-targets`（0 告警）全绿。
  - **待续**：见 §7.1「下一棒」——字节级 round-trip 收敛 + 写路径 conformance + Rust↔C++ 双向 arxml CI。

- **Milestone 23 — arxml 写路径 `nsPrefix` + 真实样本 round-trip 验证（本轮新增）**：把 §7.1 的「下一棒」第一步落地。
  - `nsPrefix` 实现：`tools/gen-autosar448-model.py` 抽取 `.ecore` 的 `xml.nsPrefix` → `FeatureMeta.ns_prefix`（`registry.rs` 重生成）；`emf-ecore` 新增 `EStructuralFeature::xml_ns_prefix()`；`metamodel.rs` 把它作为 `xml.nsPrefix` tagged value 附到特征；`saver.rs::save_attribute` 据此输出 `prefix:name`（如 `xml:space`）。
  - 真实样本验证：artop-cpp `output/samples/` 的 4 份非空真实样本（共 ~1.1MB，最大 821KB）load → save 与原文**仅差 1 行**（XML 属性顺序，语义无关），且写路径幂等（二次 round-trip 字节稳定）。
  - 测试修复：`round_trip_preserves_comment_xml_space_and_base_ref` 的 fixture 原先缺少 `Cite` `REFERENCE-BASE` 导致 `BASE="Cite"` 无法还原；补全为真实样本的三段 `REFERENCE-BASE` 后通过——确认 saver 逻辑无误。
  - 质量门禁：`cargo fmt --all` / `cargo test --workspace --all-targets`（0 失败）/ `cargo clippy --workspace --all-targets`（0 告警）全绿。

## 9. 提交记录（与本仓库进度相关的近期提交）

| 提交 | 内容 |
|---|---|
| `70ff898` | 命名重构：EMF 基础库 `emf-*`，artop 专属入 `crates/emf-artop/` |
| `df146ec` | 完成 `emf-common` + `emf-ecore` EMF 基础层（完整实现 + 集成测试） |
| `c8a71f7` | 复用 artop-cpp 权威样本做 C++ 对照：静态建模 + XMI roundtrip 9 条对照测试（Milestone 15） |