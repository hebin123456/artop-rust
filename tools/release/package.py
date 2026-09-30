#!/usr/bin/env python3
"""Package the four-platform release archives for artop-rust.

The layout mirrors the C++ reference repo (`hebin123456/artop-cpp`) release:
three archives per platform, so the three libraries the C++ release ships have a
1:1 Rust counterpart:

  artop-rust-base-<platform>-<version>.zip
      底座库 —— every generic `emf-*` rlib + the prebuilt command-line binaries
      (`arxml-roundtrip`, `arxml-validate`, `artop-codegen`, `emf-ecore-codegen`).

  artop-rust-artop-runtime-<platform>-<version>.zip
      artop-runtime 库 —— `artop_runtime` + `artop_validation` rlibs, plus the two
      arxml binaries (the "直接调用读写 arxml" entry points).

  artop-model-autosar448-<platform>-<version>.zip
      autosar448 静态模型库 —— the generated static model crate source
      (registry/lib/reflect/metamodel) + its prebuilt rlib, and `artop-codegen`
      so the registry can be regenerated.

Usage:
  python3 tools/release/package.py --release-dir target/release \
      --platform linux-x86_64 --version 1.0.0 --out dist [--exe .exe]
"""
import argparse
import glob
import os
import shutil
import sys
import zipfile

BASE_BINS = ["arxml-roundtrip", "arxml-validate", "artop-codegen", "emf-ecore-codegen"]
ARTOP_LIBS = ["artop_runtime", "artop_validation"]
MODEL_LIB = "autosar448_model"
MODEL_SRC = "crates/emf-artop/autosar448-model"


def find_rlib(release_dir, stem):
    """`target/release` holds `lib<stem>.rlib` (unix) or `<stem>.rlib` (windows)."""
    for name in (f"lib{stem}.rlib", f"{stem}.rlib"):
        path = os.path.join(release_dir, name)
        if os.path.isfile(path):
            return path
    return None


def all_rlibs(release_dir):
    return sorted(glob.glob(os.path.join(release_dir, "*.rlib")))


def zip_tree(dest, root):
    with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED) as zf:
        for base, _dirs, files in os.walk(root):
            for name in sorted(files):
                full = os.path.join(base, name)
                zf.write(full, os.path.relpath(full, root))


def stage(rel, root, src, dst):
    """Copy `root/<src>` into `<rel>/<dst>`, creating parents."""
    target = os.path.join(root, dst)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    shutil.copy2(rel, target)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--release-dir", default="target/release")
    ap.add_argument("--platform", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--out", default="dist")
    ap.add_argument("--exe", default="", help="`.exe` on windows")
    ap.add_argument("--repo-root", default=".")
    args = ap.parse_args()

    rel = args.release_dir
    if not os.path.isdir(rel):
        sys.exit(f"missing release dir: {rel}")

    suffix = f"{args.platform}-{args.version}"
    shutil.rmtree(args.out, ignore_errors=True)
    os.makedirs(args.out, exist_ok=True)

    # ---- base zip -----------------------------------------------------------
    base = os.path.join(args.out, "_base")
    for b in BASE_BINS:
        path = os.path.join(rel, b + args.exe)
        if not os.path.isfile(path):
            sys.exit(f"missing binary: {path}")
        stage(path, base, path, os.path.join("bin", b + args.exe))
    for lib in all_rlibs(rel):
        stem = os.path.basename(lib)
        if any(k in stem for k in ("artop_runtime", "artop_validation", MODEL_LIB)):
            continue
        stage(lib, base, lib, os.path.join("lib", stem))
    base_zip = os.path.join(args.out, f"artop-rust-base-{suffix}.zip")
    zip_tree(base_zip, base)

    # ---- artop-runtime zip --------------------------------------------------
    artop = os.path.join(args.out, "_artop")
    for b in ("arxml-roundtrip", "arxml-validate"):
        path = os.path.join(rel, b + args.exe)
        stage(path, artop, path, os.path.join("bin", b + args.exe))
    for stem in ARTOP_LIBS:
        path = find_rlib(rel, stem)
        if not path:
            sys.exit(f"missing rlib: {stem}")
        stage(path, artop, path, os.path.join("lib", os.path.basename(path)))
    artop_zip = os.path.join(args.out, f"artop-rust-artop-runtime-{suffix}.zip")
    zip_tree(artop_zip, artop)

    # ---- autosar448 static model zip ---------------------------------------
    model = os.path.join(args.out, "_model")
    path = find_rlib(rel, MODEL_LIB)
    if not path:
        sys.exit(f"missing rlib: {MODEL_LIB}")
    stage(path, model, path, os.path.join("lib", os.path.basename(path)))
    src_dir = os.path.join(args.repo_root, MODEL_SRC, "src")
    for name in sorted(os.listdir(src_dir)):
        if name.endswith(".rs"):
            stage(os.path.join(src_dir, name), model, os.path.join(src_dir, name),
                  os.path.join("src", name))
    stage(os.path.join(args.repo_root, MODEL_SRC, "Cargo.toml"), model,
          os.path.join(args.repo_root, MODEL_SRC, "Cargo.toml"), "Cargo.toml")
    codegen = os.path.join(rel, "artop-codegen" + args.exe)
    if os.path.isfile(codegen):
        stage(codegen, model, codegen, os.path.join("bin", "artop-codegen" + args.exe))
    model_zip = os.path.join(args.out, f"artop-model-autosar448-{suffix}.zip")
    zip_tree(model_zip, model)

    for name in (base_zip, artop_zip, model_zip):
        print(f"packaged {name} ({os.path.getsize(name)} bytes)")


if __name__ == "__main__":
    main()