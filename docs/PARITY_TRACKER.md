# EMF 底座 C++ 对照清单（PARITY TRACKER）

> 目标：把 artop-cpp `cpp/emf-cpp/emf-*` 下每个模块的 C++ 测试用例逐条移植为 Rust 对照测试，用**同一份 sample 文件**对齐行为。
> 状态图例：⬜ 未开始 · 🔶 进行中 · ✅ 已移植且跑绿 · ⛔ 无 C++ 对照 / 不移植（如 probe_arxml/probe_autosar448 属 artop 侧，按门线禁止处理）

## Hard Gate
按 `docs/EMF_PARITY_GATE.md`：**底座全部 ✅ 之前，严禁 artop。**

## emf-common（基础，最优先）
| C++ 测试 | Rust 状态 |
|---|---|
| EListTests.cpp | ⬜ |
| UniqueEListTests.cpp | ⬜ |
| BasicEMapTests.cpp | ⬜ |
| NotifyingListTests.cpp | ⬜ |
| SegmentSequenceTests.cpp | ⬜ |
| ENotifierTests.cpp | ⬜ |
| URITests.cpp | ⬜ |
| ResourceTests.cpp | ⬜ |
| CommandTests.cpp | ⬜ |
| EPackageRegistryTests.cpp | ⬜ |

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