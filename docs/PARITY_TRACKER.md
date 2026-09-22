# EMF 底座 C++ 对照清单（PARITY TRACKER）

> 目标：把 artop-cpp `cpp/emf-cpp/emf-*` 下每个模块的 C++ 测试用例逐条移植为 Rust 对照测试，用**同一份 sample 文件**对齐行为。
> 状态图例：⬜ 未开始 · 🔶 进行中 · ✅ 已移植且跑绿 · ⛔ 无 C++ 对照 / 不移植（如 probe_arxml/probe_autosar448 属 artop 侧，按门线禁止处理）

## Hard Gate
按 `docs/EMF_PARITY_GATE.md`：**底座全部 ✅ 之前，严禁 artop。**

## emf-common（基础，最优先）
> 已移植核心数据结构：`cpp_parity_common_core.rs`（25 通过 + 1 忽略）。EList / BasicEMap 全对齐；UniqueEList 语义用 BasicEList+add_unique 对齐，唯 `set` 时重复值拒绝（C++ 抛异常）记为**待补**：Rust 需专用 `UniqueEList` 类型。
| C++ 测试 | Rust 状态 |
|---|---|
| EListTests.cpp | ✅ cpp_parity_common_core |
| UniqueEListTests.cpp | 🔶 基本对齐；`set` 重复拒绝待补 |
| BasicEMapTests.cpp | ✅ cpp_parity_common_core |
| NotifyingListTests.cpp | 🔶 Rust `ListChange` hook 仅 Add/Remove/Set/Move 且不带值、无 ADD_MANY/REMOVE_MANY/MOVE 语义；需先扩展 hook（带值 + 批量变体）再完整移植，现仅数据操作可对齐 |
| SegmentSequenceTests.cpp | 🔶 `cpp_parity_common_path`；空分隔符差异待补（Rust 按字符拆 vs C++ 单段） |
| ENotifierTests.cpp | ✅ cpp_parity_common_enotifier（30；adapter 生命周期/eNotify/NotificationChain 抵消+合并+dispatch/wasSet）；⏳ 3 条 `setEContainer` 反向通知属 emf-ecore EObject 容器接线，待补 |
| URITests.cpp | ✅ cpp_parity_common_path |
| ResourceTests.cpp | 🔶 cpp_parity_common_resource（19）；setResourceSet 关联待补 |
| CommandTests.cpp | ✅ cpp_parity_common_command（39，覆盖全命令类） |
| EPackageRegistryTests.cpp | ✅ cpp_parity_ecore_registry（7；已扩展 `PackageRegistry` 增加 key 索引 get/put/contains_key/remove/keys，register 按 name/nsURI/nsPrefix 索引） |

## emf-ecore
| C++ 测试 | Rust 状态 |
|---|---|
| BasicEObjectTests.cpp | ⬜ |
| DynamicEObjectImplTests.cpp | ⬜ |
| EClassImplTests.cpp | ⬜ |
| EPackageImplTests.cpp | ⬜ |
| ETypedElementImplTests.cpp | ⬜ |
| EObjectEInvokeTests.cpp | ⬜ |
| EcorePackageTests.cpp | ⬜ |
| DataTypeUtilTests.cpp | ⬜ |
| ChangeNotificationTests.cpp | ⬜ |

## emf-xmi（序列化/反序列化）
| C++ 测试 | Rust 状态 |
|---|---|
| XMILoaderTests.cpp | 🔶 部分（元模型断言已入 codegen 对照） |
| RuntimeBehaviorTests.cpp | 🔶 部分 |
| XMISaverTests.cpp | ⬜ |
| RoundtripTests.cpp | 🔶 部分 |
| XMLHelperTests.cpp | ⬜ |
| XMIResourceFactoryTests.cpp | ⬜ |
| P3_5_GetEObjectByIDHrefTests.cpp | ⬜ |
| P3_XMLSaveLoadUUIDTests.cpp | ⬜ |
| resourceset_multi_file_test.cpp | ⬜ |
| E2E_MultiFileEcoreTests.cpp | ⬜ |
| E2E_ProxyModelTests.cpp | ⬜ |
| StaticVsDynamicXmiTests.cpp | ⬜ |
| JavaInteropTests.cpp | ⬜ |
| roundtrip_test.cpp | ⬜ |
| XmiInteropTests.cpp | ⬜ |
| E2E_GenModelXmi*.cpp（CxxProduces/EquivalentReplacement/MultiEcoreTyped/TypedMultiFile） | ⬜ |

## emf-ecore-util
| C++ 测试 | Rust 状态 |
|---|---|
| EcoreUtilTests.cpp | ⬜ |
| EcoreUtilExtendedTests.cpp | ⬜ |
| CopierTests.cpp | ⬜ |
| EcoreSwitchTests.cpp | ⬜ |
| FeatureMapTests / BasicFeatureMapTests.cpp | ⬜ |
| EObjectEListTests.cpp | ⬜ |
| EObjectValidatorTests.cpp | ⬜ |
| ValidatorComprehensiveTests.cpp | ⬜ |

## emf-edit
| C++ 测试 | Rust 状态 |
|---|---|
| EditingDomainTests.cpp | ⬜ |
| CommandTests.cpp | ⬜ |
| PlaceholderTests.cpp | ⬜ |

## emf-validation
| C++ 测试 | Rust 状态 |
|---|---|
| EValidatorTests.cpp | ⬜ |
| ConstraintParserTests.cpp | ⬜ |
| ConstraintDescriptorTests.cpp | ⬜ |
| AnnotationConstraintLoaderTests.cpp | ⬜ |
| LiveValidatorTests.cpp | ⬜ |
| ValidationServiceTests.cpp | ⬜ |
| ValidationE2ETests.cpp | ⬜ |
| AutosarConstraintsTests.cpp | ⛔ 属 artop/AUTOSAR 侧（门线禁止） |

## emf-compare
| C++ 测试 | Rust 状态 |
|---|---|
| MatchEngineTests.cpp | ⬜ |
| ComparisonTests.cpp | ⬜ |
| DiffEngineTests.cpp | ⬜ |
| MergeEngineTests.cpp | ⬜ |
| CompareE2ETests.cpp | ⬜ |
| CompareP0RegressionTests.cpp | ⬜ |

## emf-xsd
| C++ 测试 | Rust 状态 |
|---|---|
| XSDSchemaTests.cpp | ⬜ |
| XSDParserTests.cpp | ⬜ |
| XSDComponentsTests.cpp | ⬜ |
| XSDValidatorTests.cpp | ⬜ |
| XSDFacetTests.cpp | ⬜ |
| P5_XSDSchemaIncorporateTests.cpp | ⬜ |
| XSDPackageTests.cpp | ⬜ |

## emf-ecore-codegen
| C++ 测试 | Rust 状态 |
|---|---|
| GenModelLoaderTests.cpp | ✅ 已移植（cpp_parity_static_modeling） |
| RuntimeBehaviorTests.cpp | ✅ 已移植（metadata + 动态反射部分） |
| GenModelGeneratorTests.cpp | ⬜ |
| CppGeneratorTests.cpp | ⬜ |
| CppTemplatesTests.cpp | ⬜ |
| EmitterTests.cpp | ⬜ |
| P4_JetTemplateTests.cpp | ⬜ |
| TestEAnnotationReader.cpp / TestEAnnotationDebug.cpp | ⬜ |

## emf-acceleo
| C++ 测试 | Rust 状态 |
|---|---|
| AcceleoTests.cpp | ⬜ |
| AlignmentTests.cpp | ⬜ |

## emf-sphinx
| C++ 测试 | Rust 状态 |
|---|---|
| ResourceTests.cpp | ⬜ |
| ExtendedResourceTests.cpp | ⬜ |
| ModelDescriptorTests.cpp | ⬜ |
| MetaModelDescriptorTests.cpp | ⬜ |
| ScopingTests.cpp | ⬜ |
| ProxyHelperTests.cpp | ⬜ |
| EcoreResourceUtilTests.cpp | ⬜ |
| OrderedFeatureMapTests.cpp | ⬜ |

## 迁移顺序（依赖驱动）
emf-common → emf-ecore → emf-ecore-util → emf-xmi → emf-edit → emf-validation → emf-compare → emf-xsd → emf-ecore-codegen → emf-acceleo → emf-sphinx