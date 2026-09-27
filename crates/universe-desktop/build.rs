use std::path::{Path, PathBuf};
use std::process::Command;

const PREFIX: &str = "/io/github/ilyasturki/UniverseDesktop";

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    out.retain(|p| p.extension().is_some_and(|e| e == ext));
    out.sort();
    out
}

fn main() {
    println!("cargo:rerun-if-changed=data");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let data = Path::new("data");

    let blueprints = files(&data.join("ui"), "blp");
    let compiled = out.join("ui");
    std::fs::create_dir_all(&compiled).unwrap();
    let run = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(&compiled)
        .arg(data.join("ui"))
        .args(&blueprints)
        .output()
        .unwrap_or_else(|e| panic!("blueprint-compiler: {e} (it builds the .blp templates under data/ui)"));
    if !run.status.success() {
        panic!("blueprint-compiler failed:\n{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    }

    let mut entries = String::new();
    for blp in &blueprints {
        let name = blp.file_stem().unwrap().to_string_lossy();
        entries += &format!("    <file preprocess=\"xml-stripblanks\">ui/{name}.ui</file>\n");
    }
    entries += "    <file>style.css</file>\n";
    for icon in files(&data.join("icons"), "svg") {
        let name = icon.file_name().unwrap().to_string_lossy();
        entries += &format!("    <file preprocess=\"xml-stripblanks\" alias=\"icons/scalable/actions/{name}\">icons/{name}</file>\n");
    }
    let apps = data.join("icons/apps");
    for icon in files(&apps, "svg") {
        let name = icon.file_name().unwrap().to_string_lossy();
        entries += &format!("    <file preprocess=\"xml-stripblanks\" alias=\"icons/scalable/apps/{name}\">icons/apps/{name}</file>\n");
    }
    let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"{PREFIX}\">\n{entries}  </gresource>\n</gresources>\n");
    let manifest = out.join("resources.gresource.xml");
    std::fs::write(&manifest, xml).unwrap();
    glib_build_tools::compile_resources(&[out.as_path(), data], manifest.to_str().unwrap(), "universe-desktop.gresource");
}
