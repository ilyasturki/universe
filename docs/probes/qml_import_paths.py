import glob, os, subprocess

def _ldd_store_dir(so_path):
    out = subprocess.run(["ldd", so_path], capture_output=True, text=True).stdout
    dirs = set()
    for line in out.splitlines():
        if "/nix/store/" in line and "=>" in line:
            p = line.split("=>")[1].strip().split(" ")[0]
            if p.startswith("/nix/store/"):
                # /nix/store/HASH-pkg/lib/xxx.so -> /nix/store/HASH-pkg
                parts = p.split("/")
                idx = parts.index("lib") if "lib" in parts else None
                if idx:
                    dirs.add("/".join(parts[:idx]))
    return dirs

def compute_qml_import_paths():
    import PySide6
    pydir = os.path.dirname(PySide6.__file__)
    store_dirs = set()
    for mod in ("QtQml", "QtQuick", "QtMultimedia", "QtGui", "QtCore"):
        so = os.path.join(pydir, f"{mod}.abi3.so")
        if os.path.exists(so):
            store_dirs |= _ldd_store_dir(so)
    paths = []
    for d in store_dirs:
        qmldir = os.path.join(d, "lib", "qt-6", "qml")
        if os.path.isdir(qmldir):
            paths.append(qmldir)
    # qt5compat ships no python bindings -> glob candidates directly
    for d in glob.glob("/nix/store/*-qt5compat-6.11.*"):
        if any(d.endswith(s) for s in ("-dev", "-debug")) or "src" in os.path.basename(d):
            continue
        qmldir = os.path.join(d, "lib", "qt-6", "qml")
        if os.path.isdir(qmldir) and qmldir not in paths:
            paths.append(qmldir)
    return paths

if __name__ == "__main__":
    for p in compute_qml_import_paths():
        print(p)
