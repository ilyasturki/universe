import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

EXTENSION = Path(__file__).resolve().parents[1]


def interface():
    js = (EXTENSION / "extension.js").read_text()
    found = re.search(r"const IFACE = `(.*?)`;", js, re.DOTALL)
    assert found, "extension.js declares its D-Bus interface as IFACE"
    node = ET.fromstring(found.group(1))
    iface = node.find("interface")
    assert iface is not None
    methods = {}
    for method in iface.findall("method"):
        signature = {"in": "", "out": ""}
        for arg in method.findall("arg"):
            signature[arg.get("direction", "in")] += arg.get("type", "")
        methods[method.get("name")] = (signature["in"], signature["out"])
    return iface.get("name"), methods


def test_the_bus_calls_match_the_extensions_interface():
    name, methods = interface()
    gnome = (EXTENSION.parent / "crates" / "universe" / "src" / "desktop" / "gnome.rs").read_text()
    assert f'"{name}", "/org/universe/Windows", "{name}"' in gnome
    assert methods["ShowOSD"] == ("ssd", "") and '"ShowOSD", &(icon, label, level.unwrap_or(-1.0))' in gnome
    assert methods["Screenshot"] == ("sbb", "b") and '"Screenshot", &(path.to_string_lossy().as_ref(), window, cursor)' in gnome
    assert methods["List"] == ("", "s") and methods["Activate"] == ("t", "b")
    assert methods["HideCursor"] == ("b", "") and '"HideCursor", &(true,)' in gnome and '"HideCursor", &(false,)' in gnome


def test_the_metadata_names_the_shells_it_runs_on():
    meta = json.loads((EXTENSION / "metadata.json").read_text())
    assert meta["uuid"] == "universe@ilyasturki.github.io"
    assert meta["session-modes"] == ["user"]
    assert meta["shell-version"] and all(v.isdigit() for v in meta["shell-version"])
