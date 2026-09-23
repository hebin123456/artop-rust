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
| BasicEObjectTests.cpp | 🔶 cpp_parity_ecore_basic_eobject（9；eDynamic* 值存储 get/set/is_set/unset+double-feature 全对齐）；
   ⏳ eContainer / eRegisterInverseList·eInverseAdd·Remove / eSet·eUnset 触发 SET/UNSET 通知 / eNotificationRequired —— 需 Rust 层补 EObject 容器字段、inverse-list 注册表、DynamicEObject 作为 Notifier 后才可对照 |
| DynamicEObjectImplTests.cpp | ✅ cpp_parity_ecore_dyn_eobject（24；eClass / 属性 get·set·isSet·unset / 单值 containment：adopt 设 child eContainer·feature / 多值 containment：eGet 空表·增后 isSet·unset 清空 / eContents 单·多·混合收集 / 覆盖属性·child 全对齐。
   Rust 以 `DynamicEObject` 值存储 + `Weak<parent>` 容器回链 + `adopt_single`/`adopt_many` 助手模拟 C++ 指针式 `eSet(containment, child)`；差异：feature 按名访问、eContents 按 featureID 序） |
| EClassImplTests.cpp | ✅ cpp_parity_ecore_eclass（12；create/featureID/abstract·interface/get-by-ID/isSuperTypeOf/eAllSuperTypes·Attributes·References·StructuralFeatures·featureCount/ID-marked 全对齐；
   按签名对 C++ 差异：is_super_type_of 严格不自反（C++ 自反 true）、eAllOperations 继承聚合缺、getEIDAttribute 全层查找缺（Rust id_feature 仅本类）） |
| EPackageImplTests.cpp | ✅ cpp_parity_ecore_epackage（3；create/get-classifier/registry put·get·remove/accessors 全对齐） |
| ETypedElementImplTests.cpp | 🔶 cpp_parity_ecore_etyped_element（4；默认值 lower0·upper1·ordered·unique / isMany / setType→type_name / kind）；
   ⏳ eGet·eSet·eIsSet·eUnset 反射、bounds·ordered·unique setter、EGenericType（union/wildcard/懒加载同步）Rust 无对应 |
| EObjectEInvokeTests.cpp | 🔶 cpp_parity_ecore_einvoke（4；EOperation 元数据 operation_id/get-by-name/count 全对齐）；
   ⏳ eInvoke + EInvocationDelegate 派发机制 Rust 尚无对应（需对象级 invocation-delegate 注册表）|
| EcorePackageTests.cpp | ✅ cpp_parity_ecore_package（6；ePackage 周期初始化/19 元类·18 内建数据类型注册/全局 registry 按 nsURI·prefix 索引/nsURI·prefix·name 常量/FeatureID 常量块（EClass·EPackage）全对齐。
   差异：Rust 以 find_class·find_data_type 替代 getEClass_EClass 式类型化 getter） |
| DataTypeUtilTests.cpp | ✅ cpp_parity_ecore_datatype_util（10；EString·EInt·EBoolean·EDouble from/to、默认值 EString""·EInt0·EBooleanfalse·EDouble0.0·ELong0、coerce（string→int / int→string / int→bool）、nsURI 常量 全对齐。
   Rust 以 `Val` 替代 std::any，coerce 用 from_string(to_string(...)) 表达） |
| ChangeNotificationTests.cpp | ✅ 已由 emf-common `cpp_parity_common_enotifier`（30 条）覆盖：Notification 构造/访问器/EventType 值·名称/position/touch·wasTouched/NotificationChain 聚合/多 adapter 广播/removeAdapter REMOVING_ADAPTER 隐式通知/target 管理 全部对齐 |

## emf-xmi（序列化/反序列化）
| C++ 测试 | Rust 状态 |
|---|---|
| XMILoaderTests.cpp | 🔶 部分（元模型断言已入 codegen 对照） |
| RuntimeBehaviorTests.cpp | 🔶 部分 |
| XMISaverTests.cpp | ✅ cpp_parity_xmi_saver（13；EPackage 元模型文档：空根/包元数据 name·nsURI·nsPrefix/ecore:EPackage·xmi·xsi 命名空间声明/EClass·EAttribute·EReference·EEnum 输出/eType 内建 href·`#//同包` 形式/upperBound·containment·abstract·iD·resolveProxies/jsonEncoding·xmlDeclaration·xmiVersion 选项/EEnum 自增 value="" 省略 全对齐。
   Rust 新增 `emf_xmi::metamodel_saver`（EPackage→<ecore:EPackage>）补齐元模型序列化） |
| RoundtripTests.cpp | ✅ cpp_parity_xmi_roundtrip（9；load→save→reload 后 name·nsURI·nsPrefix/classifier 数·名/feature name·type·containment·upperBound/defaultValueLiteral/两次 save 幂等/EEnum literal name·value·literal/abstract·eSuperTypes/空包幂等/iD·resolveProxies 保持 全对齐。
   loader 侧同步补齐：EEnum literal 缺省 value 按序取值、iD/resolveProxies 属性解析） |
| XMLHelperTests.cpp | ✅ cpp_parity_xml_helper（32；XMIHelper 工具函数 splitQName/splitHref/stripFragmentSlash、escapeXmlAttr（`& < "`·`\n\r\t`→`&#xA;/&#xD;/&#x9;`·`>`与`'`不转义·非 ASCII→`&#xNNNN;` 小写、mappableLimit 默认 0x7F/passthrough 0x10FFFF）、escapeXmlText（保留 `\n \t`，转义 `\r`）、四个命名空间常量 kEcore·kXmi·kXmi2·kXsi/XMLHelperImpl 命名空间上下文 getURI·getPrefix·getNamespaceURI·pop 清除·嵌套作用域/getXMLEncoding·getJavaEncoding 编码映射/setResource·setNoNamespacePackage·setBaseURI/FeatureKind 常量 1-5 全对齐。
   Rust 新增 `emf_xmi::xmi_helper` 模块并扩展 `XMLHelper` 补齐 resource·base_uri·编码映射与 get_namespace_uri） |
| XMIResourceFactoryTests.cpp | ✅ cpp_parity_xmi_resource_factory（4；registerDefaults 使 .xmi/.ecore 可创建/未知扩展回退 XMIResource（不抛异常）/registerFactory 自定义扩展·大小写不敏感/direct createResource 返回具体 XMIResource 全对齐。
   Rust 在 `XMIResourceFactory` 补齐静态扩展分派 API：thread_local 扩展注册表·register_defaults·register_factory·create_resource_for·create_resource） |
| P3_5_GetEObjectByIDHrefTests.cpp | ✅ cpp_parity_get_eobject_by_id（13；setID/getID/getEObjectByID 双向映射·覆盖旧 id·未知 id→None·未注册→""/xmi:id 加载自动注册/getEObject 三种形态 "?id"·"Name"·"//Name"/空·未知 fragment→None/resolvePositionPath "@feat.index"（含多值 containment 取首个/次个）·无 @ 按名/getIDToEObjectMap 全部注册 id 全对齐。
   Rust 在 `XMIResource` 补齐 id 双向映射（Rc 指针键）+ 片段导航，`loader::load_from_str_with_ids` 导出 xmi:id 表；C++ 走 ecore:EPackage 元模型、Rust 以 DynamicEObject 文档承载同一 ID/href 契约） |
| P3_XMLSaveLoadUUIDTests.cpp | ✅ cpp_parity_xml_save_load_uuid（9；generateUUID v4 格式 36 字符·8-4-4-4-12·第14位'4'·第19位[89ab]/1000 次唯一/ensureID：useUUIDs=false 不分配·true 自动分 UUID·幂等·写入 idToEObjectMap 且可反查/XMLSave 注入自定义实现被 save 调用/XMLoad 注入自定义实现被 load 调用/默认 getXMLSave·getXMLLoad 非空·缓存同一实例·端到端默认保存出真实 XMI 全对齐。
   Rust 在 `XMIResource` 补齐 v4 UUID 生成（splitmix64+单调序号）、`use_uuids`/`ensure_id`，并新增 `emf_xmi::xml_save_impl` 模块（XMLSave/XMLSaveImpl + XMLLoader/XMLoaderImpl 注入抽象），`save_to_string`/`load_from_string` 经当前激活实现分派） |
| resourceset_multi_file_test.cpp | ✅ cpp_parity_resourceset_multi_file（1；场景A：a.xmi(EPackage pkgA+EClass A)·b.xmi(pkgB+EClass B, eSuperTypes="a.xmi#//A") 分别 load_ecore_package→EPackage，共注册入同一 PackageRegistry 后 B.eSuperTypes 解析为真实 EClass A（非 proxy，相当于 EMF 跨包物件解析）全对齐。
   差异：场景B（arxml AutosarResourceSet）属 artop 侧按门线禁止；场景A 的 ResourceSet 自动 demand-load 由 object-proxy 触发，Rust 元模型层无持久 proxy 对象，以「跨包按名解析到已注册真实 classifier」等价表达） |
| E2E_MultiFileEcoreTests.cpp | ✅ e2e_multi_file_ecore（10；base .ecore(EPackage base+Library/Book/Writer) 按 nsURI 注册可查/ext .ecore 加载后结构（AnnotatedLibrary·BookCollection 2 classifier）/跨包 eSuperTypes：AnnotatedLibrary 继承 base#//Library 解析到真实 Library/跨包 eType：highlighted·books 均解析到 base#//Book（Reference·many·containment·type_name=Book）/双包同存 Registry（base·ext 双 nsURI 均在）/samples/multi/library.ecore·library_ext.ecore 磁盘加载 + 跨文件 superType·eType 解析 全对齐。
   样本文件已随 crate 归置于 tests/samples/multi/；C++ 走 EPackageRegistry object-proxy 解析、Rust 以「type_name/href_tail 按名 + 共享 registry 解析到真实 class」等价。MISS 时静默跳过（与 C++ 一致）） |
| E2E_ProxyModelTests.cpp | ✅ cpp_parity_proxy_model（13；新对象默认非代理/设置 eSetProxyURI 后 eIsProxy==true/eProxyURI 返回所设 URI/eResolveProxy 非代理返回自身/经 ResourceSet 解析代理达到 target 根对象（非代理自身）/无 ResourceSet 时退化返回代理自身/带 fragment 代理 URI 整体存储且 fragment() 返回 `//Library/books.0`/多代理各自独立 URI/getEObject(uri,loadOnDemand) 跨资源按 URI 找根/getResource(uri,false) 返回已注册资源/eIsProxy 设置后持续为 true/后设代理 URI 覆盖先前 全对齐。
   Rust 在 `EObject` trait 与 `DynamicEObject` 补齐 `proxy_uri`/`e_set_proxy_uri`/`e_is_proxy`/`e_resolve_proxy`（默认非代理解析自身·无 set 退化返回 proxy），并新增 `emf_xmi::xmi_resource_set`（XMIResourceSet：create_resource/get_resource/get_eobject(按 URI 去 fragment 找根)/resolve_proxy_uri，等价 EMF ResourceSet.getEObject + EcoreUtil.resolve） |
| StaticVsDynamicXmiTests.cpp | ✅ cpp_parity_static_vs_dynamic_xmi（3;动态 DynamicEObject 构造 Library{name}+2×Book{title,pages} 产出 XMI 含 name="Test Library"·title="Book One/Two"·pages="100/200" 结构/reload 该 XMI roundtrip 后 name·title·pages 与 2 本 books 保持/静态与动态两路径规范化 XMI 语义一致·containment `<books>` 子元素恰好 2 个 全对齐。
   Rust"静态"与"动态"同为反射后端，等价性天然成立，以「两次独立重建文档 normalize 后相等 + books 子元素计数一致」等价表达 C++ 的 normalizeXml+countOccurrences） |
| JavaInteropTests.cpp | ✅ cpp_parity_java_interop（3；Java 风格独立 `<ecore:EPackage>` 文档（无需外部包注册）直读后 name="library"·nsURI·nsPrefix + ≥4 classifier（Library·Book 类 + BookCategory 枚举3 literal + MyString EDataType）；写回时恰好单个 `<ecore:EPackage>` 包裹且保留 nsURI·name·nsPrefix，并重输出 Library/BookCategory 及 `xsi:type="ecore:EClass|EEnum|EDataType"` 标记 全对齐。
   差异：C++ 读 `/workspace/emf-cpp-demo/build/java_ref/library.ecore`（Java 生成参考文件），该文件不在仓库（源 C++ 亦在缺文件时 skip）；Rust 以同构 Java 风格独立多 classifier 文档承载同一互读/写回契约） |
| roundtrip_test.cpp | ⚪ 不可移植（ARXML CLI 工具）|
| XmiInteropTests.cpp | ✅ cpp_parity_xmi_interop（5；非 containment 跨引用指向 containment 树内对象时序列化为 Java 兼容 position-path `author="//@writers.0"` 且非裸 `//`·并声明 xmlns:xsi/save→load→再读保持跨引用·读取 Java 风格属性 position-path `author="//@writers.0"`·读取空格分隔多值跨引用 `authors="//@writers.0 //@writers.1"`→2 个 Writer·读取 `xmi:id` 寻址 `author="//w1"`→`<writers xmi:id="w1">` 全对齐。
   差异/Rust 增强：C++ Saver 对 containment 树内对象生成 `//@feat.idx`；Rust 在 `saver::index_tree_positions` 预索引 containment 树、跨引用优先 position-path（树外对象回退 `//<xmi:id>`），loader 侧 `index_positions` 建 `path→object` 索引直接解析 `//@...`，并支持 href 空格分词多值（`resolve_token`）。`roundtrip_test.cpp` 为 ARXML 往返 CLI（加载 autosar40 424 静态包子包 + Java 参考文件对比），依赖 Autosar39 静态 codegen 与 `/tmp/*.arxml` 外部输入，非通用 EMF XMI 运行时契约，不移植 |
| E2E_GenModelXmiEquivalentReplacementTests.cpp | ✅ cpp_parity_e2e_equiv（10；Java 参考 library.ecore 独立文档等价替换：name·nsURI·nsPrefix/classifier 数·名/library.ecore 读进 + save-back 含 `xsi:type="ecore:EClass"`·`eType="...#//EString"`·containment="true"/多包共享注册/跨包 eType·superType 按名解析到真实 class/往返 defaultValueLiteral="0" 保持/Java 样例文件解析回再写出保留 nsURI 全对齐。
   「等价替换」语义：C++ 用真实 Java 生成文件做互读互写，Rust 以同构 Java 风格独立文档承载同一契约） |
| E2E_GenModelXmiCxxProducesTests.cpp | ✅ cpp_parity_e2e_genmodel_xmi（10；gen-model 全链路：注入元模型→动态实例化 Library/Book/Writer→存属性(name·title·pages int)→设置 containment books/library↔book 关系→save→reload 后 Book 类名·title·pages(Int) 保持/重复 reload 幂等 save 一致/XML 头与 xmlns 声明/缺省 writer author 关联 全对齐。
   Rust 复用「load_ecore_package→registry→DynamicEObject 实例化→saver/loader」；C++ 用 Fjage 组件产出的静态类，契约等价） |
| E2E_GenModelXmiMultiEcoreTypedTests.cpp | ✅ cpp_parity_e2e_multi_ecore_typed（12；base(Library:name+books / Book:title)+ext(AnnotatedLibrary extends base#//Library 增 note 属性 + highlighted containment of base#//Book) 跨包 typed：AnnotatedLibrary 继承 Library/supertype 解析到真实 base Library/自有 feature==2 {note,highlighted}/eAllStructuralFeatures≥4 含继承 {name,books}+自有 {note,highlighted}·全部按名可查/实例化 eClass()=="AnnotatedLibrary"/反射设继承 name 与自有 note·读回/add Book 到继承 books 列表与自有 highlighted 列表/跨包 containment highlighted.eType 解析 base#//Book+eAllContainments 含 books+highlighted/save 产出 `ext:AnnotatedLibrary`·name·note 字段/两包各自实例化基类对象 全对齐。
   修复 Rust 跨包缺陷：`e_all_structural_features` 与 `DynamicEObject` 值/标志存储原按 feature_id（每包从 0 编号→跨包冲突）改为按 feature 名键控；e_all 去重改按名，存储改按名，修复继承的 `name` 与自有 `note` 同 id0 互相覆盖的 bug） |
| E2E_GenModelXmiTypedMultiFileTests.cpp | ✅ cpp_parity_e2e_typed_multi_file（12；加载内联 ecore（6 classifier·nsURI `http://example.com/emfdemo/library`·含 Library 5 containment books/magazines/authors/publishers）并注册/library.xmi 单根 `<library:Library>`·name=="City Central Library"/books containment 2·首 title=="The Pragmatic Programmer"/authors containment 3·首 name=="Ada Lovelace"/publishers containment 2·首 name=="O'Reilly Media"/authors.xmi `<xmi:XMI>` 多根·3 个 Author 根·name·email(`ada@example.com`) 加载/publishers.xmi 多根·2 个 Publisher·嵌套单值 address containment·city=="Sebastopol"/多文件独立加载互不干扰/各文件实例 class 均经同一注册元模型包(nsURI)解析 全对齐。
   样本已随 crate 归置 tests/samples/multi-xmi-java/（Java 参考 library.xmi·authors.xmi·publishers.xmi）；Java 文件带元模型未声明属性(publishDate/category/...)由 loader record-and-skip。差异：C++ `getEPackage` 指针恒等，Rust 以「find_package_of_class→nsURI==emfdemo/library」等价）。emf-xmi GenModel*.cpp 全系列已移植完毕 |

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