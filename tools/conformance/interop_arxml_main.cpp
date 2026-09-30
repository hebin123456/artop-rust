// interop_arxml_main.cpp —— 双向 ARXML 互操作 harness 的 C++ 半侧。
//
// 供 tools/conformance/interop_arxml.py 驱动，证明 Rust 与 C++ 两个移植版本能
// 互相读写对方的 arxml（而不只是"对同一批 fixture 行为一致"）：
//
//   interop_arxml roundtrip <in.arxml> <out.arxml>   读入后原样写回
//   interop_arxml check     <in.arxml>               读入并校验约定的根结构
//
// 元模型注册（按优先级）：
//   1. 若设置 ARXML_INTEROP_ECORE_448（+ 可选 ARXML_INTEROP_ECORE_GAUTOSAR），
//      运行时动态加载完整 AUTOSAR448 .ecore（与 Rust 侧 autosar448-model 同源）；
//   2. 否则回退到 C++ 内置的最小 autosar40.ecore。
// 动态路径把根包额外注册到裸 nsURI "http://autosar.org/schema/r4.0"
// （AutosarXMLLoader 按该 nsURI 查根包，而 autosar448.ecore 的根包 nsURI 带
//  "/autosar40" 后缀）。
//
// 注意（C++ 参考实现的当前范围）：artop-cpp 的 artop-runtime 默认只带一份
// *最小* autosar40.ecore（AUTOSAR/SHORT-NAME/AR-PACKAGE 级），其注释也写明
// 「完整对齐时可用 codegen 生成全量静态包替换此文件」。因此本 harness 的
// `roundtrip` 在 C++ 侧**不保证**与原文逐字节相同（Rust 侧才是逐字节的）；
// `check` 只断言「能读成 AUTOSAR 根 + 至少一个 AR-PACKAGE」，作为跨实现
// 文件交接的验收条件。
//
// 任一步失败时以非 0 退出并打印 INTEROP-FAIL 原因。
#include "emf/artop/runtime/AutosarResource.h"
#include "emf/artop/runtime/AutosarResourceFactory.h"
#include "emf/xmi/XMIResource.h"
#include "emf/xmi/XMIResourceFactory.h"
#include "emf/xmi/XMIOptions.h"
#include "emf/ecore/EcorePackage.h"
#include "emf/ecore/EcoreImpls.h"
#include "emf/common/EPackageRegistry.h"
#include "emf/common/EList.h"

#include <cstdlib>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

using emf::artop::runtime::AutosarResourceFactory;
using emf::artop::runtime::AutosarXMLResource;
using emf::common::EObject;
using emf::ecore::EClass;
using emf::ecore::EPackage;
using emf::ecore::EStructuralFeature;
using emf::xmi::XMIOptions;

namespace {

constexpr const char* kAutosarNsURI = "http://autosar.org/schema/r4.0";
constexpr const char* kAutosarRootElement = "AUTOSAR";
constexpr const char* kArPackagesFeature = "arPackages";
constexpr const char* kArPackageElement = "AR-PACKAGE";

std::string envOr(const char* name, const std::string& def = "") {
    const char* v = std::getenv(name);
    return v ? std::string(v) : def;
}

std::string readFile(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) throw std::runtime_error("cannot open input file: " + path);
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

void writeFile(const std::string& path, const std::string& data) {
    std::ofstream out(path, std::ios::binary);
    if (!out) throw std::runtime_error("cannot open output file: " + path);
    out << data;
}

// 从 arxml 根元素里取 xsi:schemaLocation，保证写回时与原文件一致。
std::string extractSchemaLocation(const std::string& xml) {
    const std::string key = "xsi:schemaLocation=\"";
    auto pos = xml.find(key);
    if (pos == std::string::npos) return "";
    pos += key.size();
    auto end = xml.find('"', pos);
    if (end == std::string::npos) return "";
    return xml.substr(pos, end - pos);
}

void initEnv() {
    emf::ecore::EcoreFactory::initialize();
    emf::ecore::EcorePackage::initialize();
    emf::xmi::XMIResourceFactory::registerDefaults();
}

// 动态加载的 .ecore Resource 必须存活，否则其 EPackage 会变成悬空指针。
std::vector<std::unique_ptr<emf::xmi::XMIResource>>& ecoreKeepAlive() {
    static std::vector<std::unique_ptr<emf::xmi::XMIResource>> v;
    return v;
}

// 独立加载一个 .ecore（不挂 ResourceSet）。
//
// 关键：不挂 ResourceSet 时，XMILoader 对 `gautosar.ecore#//...` 这类跨文件
// eSuperTypes 会解析失败并安全跳过（resolveCrossDocClassifier 在无 ResourceSet
// 时返回 nullptr），从而得到与 C++ 静态生成模型一致的、仅含 autosar448 自身
// feature 的 EClass 图。若挂了 ResourceSet，loader 会去 demand-load 相对路径下的
// gautosar.ecore，并把 gautosar 的抽象 feature 混入 AUTOSAR 类，破坏元素映射。
// 递归给包树里每个缺 EFactory 的 EPackage 补一个 EFactory。
//
// 静态生成模型里每个包都自带 Factory；动态加载的 .ecore 只让根包含 Factory。
// AutosarXMLLoader 设置 EAttribute 值时走 `eType->getEPackage()->getEFactoryInstance()`
// 且不回落父包，因此子包缺 Factory 会导致所有属性值被静默丢弃。
void assignFactories(EPackage* pkg) {
    if (!pkg) return;
    if (!pkg->getEFactoryInstance()) {
        auto* f = emf::ecore::EcoreFactory::instance().createEFactory();
        f->setEPackage(pkg);
        pkg->setEFactoryInstance(f);
    }
    for (auto* sp : pkg->getESubpackages()) assignFactories(sp);
}

EPackage* loadEcoreStandalone(const std::string& path) {
    auto uri = emf::common::URI::createFileURI(path);
    auto res = emf::xmi::XMIResourceFactory::createResourceFor(uri);
    if (!res) throw std::runtime_error("cannot create resource for " + path);
    std::ifstream ifs(path);
    if (!ifs.is_open()) throw std::runtime_error("cannot open " + path);
    res->load(ifs);
    if (res->getContents().empty()) throw std::runtime_error("empty ecore: " + path);
    auto* pkg = dynamic_cast<EPackage*>(res->getContents()[0]);
    if (!pkg) throw std::runtime_error("root is not EPackage: " + path);
    // 为整棵包树补 EFactory（对齐静态生成模型里每包自带 Factory）
    assignFactories(pkg);
    ecoreKeepAlive().push_back(std::move(res));
    return pkg;
}

void registerMetamodel() {
    std::string gautosar = envOr("ARXML_INTEROP_ECORE_GAUTOSAR");
    std::string autosar448 = envOr("ARXML_INTEROP_ECORE_448");
    if (autosar448.empty()) {
        // 回退：内置最小 autosar40.ecore（只能处理 AR-PACKAGE 级结构）。
        if (!AutosarResourceFactory::registerDefaultAutosar40Metamodel()) {
            throw std::runtime_error(
                "no metamodel: set ARXML_INTEROP_ECORE_448 or build with "
                "EMF_ARTOP_AUTOSAR40_ECORE");
        }
        return;
    }
    // gautosar 独立加载并只按自身 nsURI 注册（不 grounding 到 autosar448 的
    // eSuperTypes，见 loadEcoreStandalone 注释）。
    if (!gautosar.empty()) {
        if (auto* gpkg = loadEcoreStandalone(gautosar)) {
            emf::common::EPackageRegistry::instance().put(gpkg->getNsURI(), gpkg);
        }
    }
    auto* pkg = loadEcoreStandalone(autosar448);
    emf::common::EPackageRegistry::instance().put(pkg->getNsURI(), pkg);
    // AutosarXMLLoader 按裸 nsURI 查根包；autosar448.ecore 根包 nsURI 带
    // "/autosar40" 后缀，这里补注册一份裸 nsURI（同一 EPackage 对象）。
    emf::common::EPackageRegistry::instance().put(kAutosarNsURI, pkg);
}

// 泄漏式持有：动态加载的元模型 + arxml 对象图在析构时存在所有权交叉
// （harness 只关心读写结果，进程随即退出），故不回收。
AutosarXMLResource* loadResource(const std::string& path) {
    std::string xml = readFile(path);
    auto* res = new AutosarXMLResource(emf::common::URI::createFileURI(path));
    std::string sl = extractSchemaLocation(xml);
    if (!sl.empty()) res->setSchemaLocation(sl);

    XMIOptions opts;
    std::istringstream iss(xml);
    res->load(iss, opts);
    return res;
}

int roundtrip(const std::string& inPath, const std::string& outPath) {
    initEnv();
    registerMetamodel();
    auto res = loadResource(inPath);
    std::ostringstream oss;
    res->save(oss, XMIOptions{});
    writeFile(outPath, oss.str());
    std::cout << "ROUNDTRIP-OK " << outPath << std::endl;
    return 0;
}

// 多值 containment 元素个数（兼容 EList / vector 两种 eGet 返回类型）。
size_t manySize(EObject* obj, EStructuralFeature* sf) {
    auto v = obj->eGet(sf);
    if (auto* elist = std::any_cast<emf::common::EList<EObject*>*>(&v)) {
        if (*elist) return (*elist)->size();
    } else if (auto* listPtr = std::any_cast<std::vector<EObject*>*>(&v)) {
        if (*listPtr) return (*listPtr)->size();
    }
    return 0;
}

int check(const std::string& inPath) {
    initEnv();
    registerMetamodel();
    auto res = loadResource(inPath);
    auto& contents = res->getContents();
    if (contents.empty()) throw std::runtime_error("no root object");

    auto* root = contents.front();
    auto* cls = root->eClass();
    if (!cls) throw std::runtime_error("root object has no EClass");
    if (cls->getName() != kAutosarRootElement) {
        throw std::runtime_error("root class `" + cls->getName() +
                                 "` != `" + kAutosarRootElement + "`");
    }
    // AR-PACKAGE 集合：完整动态元模型里 feature 名是 ecore 名 `arPackages`，
    // 内置最小 autosar40.ecore 里直接用 arxml 元素名 `AR-PACKAGE`，两者都试。
    EStructuralFeature* sf = cls->getEStructuralFeature(kArPackagesFeature);
    if (!sf) sf = cls->getEStructuralFeature(kArPackageElement);
    if (!sf) throw std::runtime_error("AUTOSAR has no AR-PACKAGE feature");
    size_t n = manySize(root, sf);
    if (n == 0) throw std::runtime_error("AUTOSAR has no AR-PACKAGE child");

    std::cout << "CHECK-OK " << inPath << " (AR-PACKAGE=" << n << ")" << std::endl;
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    int rc;
    try {
        if (argc == 4 && std::string(argv[1]) == "roundtrip")
            rc = roundtrip(argv[2], argv[3]);
        else if (argc == 3 && std::string(argv[1]) == "check")
            rc = check(argv[2]);
        else {
            std::cerr << "usage: interop_arxml <roundtrip> <in.arxml> <out.arxml>"
                         " | <check> <in.arxml>" << std::endl;
            rc = 2;
        }
    } catch (const std::exception& e) {
        std::cerr << "INTEROP-FAIL: " << e.what() << std::endl;
        rc = 1;
    }
    // 动态加载的 .ecore 及其注册表在进程退出时的静态析构顺序不可控，会触发
    // "double free or corruption"（结果已产出，仅退出阶段崩溃）。作为一次性
    // harness，直接 _Exit 跳过静态析构，保证退出码反映真实结果。
    std::cout.flush();
    std::cerr.flush();
    std::_Exit(rc);
}