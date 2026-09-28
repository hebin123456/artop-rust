// interop_xmi_main.cpp —— 双向 XMI 互操作 harness 的 C++ 半侧。
//
// Rust 与 C++ 两侧构建同一个 `library` 元模型 + 同一份实例数据；本程序是
// C++ 侧，供 tools/conformance/interop_xmi.py 驱动，用于证明两个移植版本能
// 互相读写对方的 XMI 文件（而不只是"对同一批 fixture 行为一致"）。
//
//   interop_xmi emit      <out.xmi>          构建模型并以 XMI 落盘
//   interop_xmi check     <in.xmi>           读入任一侧产出的 XMI 并校验约定形状
//   interop_xmi roundtrip <in.xmi> <out.xmi> 读入后再写回（跨侧读→写）
//
// 任一步失败时以非 0 退出并打印 INTEROP-FAIL 原因。
#include "emf/xmi/XMIResource.h"
#include "emf/xmi/XMIResourceFactory.h"
#include "emf/ecore/EcorePackage.h"
#include "emf/ecore/EcoreImpls.h"
#include "emf/common/EList.h"
#include "emf/common/EObject.h"

#include <any>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

using emf::xmi::XMIResource;
using emf::xmi::XMIResourceFactory;
using emf::ecore::EcoreFactory;
using emf::ecore::EcorePackage;
using emf::ecore::EPackage;
using emf::ecore::EClass;
using emf::ecore::EFactory;
using emf::ecore::EStructuralFeature;
using emf::common::EObject;

namespace {

// 与 Rust 侧 `K_LIBRARY_ECORE` / C++ `XmiInteropTests.cpp` 保持同一元模型。
const char* kLibraryEcore =
    "<?xml version=\"1.0\"?>\n"
    "<ecore:EPackage xmlns:ecore=\"http://www.eclipse.org/emf/2002/Ecore\" "
    "xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" "
    "xmi:version=\"2.0\" name=\"library\" nsURI=\"http://example.com/library/1.0\" nsPrefix=\"library\">"
    "<eClassifiers xsi:type=\"ecore:EClass\" name=\"Library\">"
    "<eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"name\" "
    "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>"
    "<eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"books\" upperBound=\"-1\" "
    "eType=\"#//Book\" containment=\"true\"/>"
    "<eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"writers\" upperBound=\"-1\" "
    "eType=\"#//Writer\" containment=\"true\"/>"
    "<eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"address\" "
    "eType=\"#//Address\" containment=\"true\"/>"
    "</eClassifiers>"
    "<eClassifiers xsi:type=\"ecore:EClass\" name=\"Book\">"
    "<eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"title\" "
    "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>"
    "<eStructuralFeatures xsi:type=\"ecore:EReference\" name=\"author\" "
    "eType=\"#//Writer\"/>"
    "</eClassifiers>"
    "<eClassifiers xsi:type=\"ecore:EClass\" name=\"Writer\">"
    "<eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"name\" "
    "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>"
    "</eClassifiers>"
    "<eClassifiers xsi:type=\"ecore:EClass\" name=\"Address\">"
    "<eStructuralFeatures xsi:type=\"ecore:EAttribute\" name=\"street\" "
    "eType=\"ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString\"/>"
    "</eClassifiers>"
    "</ecore:EPackage>";

// 两侧约定的实例数据。
const char* kLibName = "Interop Lib";
const char* kBookTitle = "B1";
const char* kWriterName = "W1";

void initEnv() {
    EcoreFactory::initialize();
    EcorePackage::initialize();
    XMIResourceFactory::registerDefaults();
}

struct ModelMeta {
    EPackage* pkg = nullptr;
    EClass* libCls = nullptr;
    EClass* bookCls = nullptr;
    EClass* writerCls = nullptr;
    EFactory* factory = nullptr;
};

ModelMeta loadModel() {
    XMIResource res;
    res.loadFromString(std::string(kLibraryEcore));
    ModelMeta m;
    m.pkg = dynamic_cast<EPackage*>(res.getContents().front());
    res.getContents().clear();
    m.libCls = dynamic_cast<EClass*>(m.pkg->getEClassifier("Library"));
    m.bookCls = dynamic_cast<EClass*>(m.pkg->getEClassifier("Book"));
    m.writerCls = dynamic_cast<EClass*>(m.pkg->getEClassifier("Writer"));
    m.factory = m.pkg->getEFactoryInstance();
    return m;
}

// 多值 containment 追加（对齐 emf_test::addToContainment：取出→追加→eSet 回写）。
void addToContainment(EObject* obj, EStructuralFeature* sf, EObject* value) {
    std::vector<EObject*> v;
    auto any = obj->eGet(sf);
    if (any.has_value()) {
        if (any.type() == typeid(emf::common::EList<EObject*>*)) {
            auto* elist = std::any_cast<emf::common::EList<EObject*>*>(any);
            if (elist) {
                for (size_t i = 0; i < elist->size(); ++i) v.push_back((*elist)[i]);
            }
        } else if (any.type() == typeid(std::vector<EObject*>)) {
            v = std::any_cast<std::vector<EObject*>>(any);
        } else if (any.type() == typeid(std::vector<EObject*>*)) {
            auto* p = std::any_cast<std::vector<EObject*>*>(any);
            if (p) v = *p;
        }
    }
    v.push_back(value);
    obj->eSet(sf, std::any(std::move(v)));
}

std::string getStr(EObject* obj, EClass* cls, const std::string& feat) {
    auto* sf = cls->getEStructuralFeature(feat);
    if (!sf) return "";
    auto v = obj->eGet(sf);
    if (v.type() == typeid(std::string)) return std::any_cast<std::string>(v);
    return "";
}

EObject* getRef(EObject* obj, EClass* cls, const std::string& feat) {
    auto* sf = cls->getEStructuralFeature(feat);
    if (!sf) return nullptr;
    auto v = obj->eGet(sf);
    if (v.type() == typeid(EObject*)) return std::any_cast<EObject*>(v);
    return nullptr;
}

emf::common::EList<EObject*>* getMany(EObject* obj, EClass* cls,
                                      const std::string& feat) {
    auto* sf = cls->getEStructuralFeature(feat);
    if (!sf) return nullptr;
    auto v = obj->eGet(sf);
    if (v.type() == typeid(emf::common::EList<EObject*>*)) {
        return std::any_cast<emf::common::EList<EObject*>*>(v);
    }
    return nullptr;
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

int emitModel(const std::string& outPath) {
    initEnv();
    auto m = loadModel();

    auto* lib = m.factory->create(m.libCls);
    lib->eSet(m.libCls->getEStructuralFeature("name"), std::any(std::string(kLibName)));

    auto* book = m.factory->create(m.bookCls);
    book->eSet(m.bookCls->getEStructuralFeature("title"), std::any(std::string(kBookTitle)));

    auto* writer = m.factory->create(m.writerCls);
    writer->eSet(m.writerCls->getEStructuralFeature("name"), std::any(std::string(kWriterName)));

    // Book.author 是非 containment 跨引用；writer 同时挂在 lib.writers（containment）
    // 下，因此序列化应产出 Java 兼容的位置路径 `//@writers.0`。
    book->eSet(m.bookCls->getEStructuralFeature("author"), std::any((EObject*)writer));
    addToContainment(lib, m.libCls->getEStructuralFeature("books"), book);
    addToContainment(lib, m.libCls->getEStructuralFeature("writers"), writer);

    XMIResource res;
    res.getContents().push_back(lib);
    writeFile(outPath, res.saveToString());
    std::cout << "EMIT-OK " << outPath << std::endl;
    return 0;
}

int checkModel(const std::string& inPath) {
    initEnv();
    loadModel();  // 注册 EPackage，使实例文档能解析类名 / 跨引用

    XMIResource res;
    res.loadFromString(readFile(inPath));
    if (res.getContents().empty()) throw std::runtime_error("no root object");

    auto* lib = res.getContents().front();
    auto* libCls = lib->eClass();
    std::string gotName = getStr(lib, libCls, "name");
    if (gotName != kLibName)
        throw std::runtime_error("root name `" + gotName + "` != `" + kLibName + "`");

    auto* books = getMany(lib, libCls, "books");
    if (!books || books->size() != 1u)
        throw std::runtime_error("expected exactly 1 book");
    auto* book = (*books)[0];
    auto* bookCls = book->eClass();
    std::string gotTitle = getStr(book, bookCls, "title");
    if (gotTitle != kBookTitle)
        throw std::runtime_error("book title `" + gotTitle + "` != `" + kBookTitle + "`");

    auto* author = getRef(book, bookCls, "author");
    if (!author) throw std::runtime_error("author cross-reference not resolved");
    std::string gotAuthor = getStr(author, author->eClass(), "name");
    if (gotAuthor != kWriterName)
        throw std::runtime_error("author name `" + gotAuthor + "` != `" + kWriterName + "`");

    std::cout << "CHECK-OK " << inPath << std::endl;
    return 0;
}

int roundtripModel(const std::string& inPath, const std::string& outPath) {
    initEnv();
    loadModel();

    XMIResource res;
    res.loadFromString(readFile(inPath));
    writeFile(outPath, res.saveToString());
    std::cout << "ROUNDTRIP-OK " << outPath << std::endl;
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    try {
        if (argc == 3 && std::string(argv[1]) == "emit")
            return emitModel(argv[2]);
        if (argc == 3 && std::string(argv[1]) == "check")
            return checkModel(argv[2]);
        if (argc == 4 && std::string(argv[1]) == "roundtrip")
            return roundtripModel(argv[2], argv[3]);
        std::cerr << "usage: interop_xmi <emit|check> <file.xmi>"
                     " | <roundtrip> <in.xmi> <out.xmi>" << std::endl;
        return 2;
    } catch (const std::exception& e) {
        std::cerr << "INTEROP-FAIL: " << e.what() << std::endl;
        return 1;
    }
}
