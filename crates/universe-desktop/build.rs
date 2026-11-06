use std::path::{Path, PathBuf};
use std::process::Command;

const PREFIX: &str = "/io/github/ilyasturki/UniverseDesktop";

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    out.retain(|p| p.extension().is_some_and(|e| e == ext));
    out.sort();
    out
}

fn run(mut command: Command) {
    let tool = command.get_program().to_string_lossy().into_owned();
    let run = command.output().unwrap_or_else(|e| panic!("{tool}: {e} (it builds the templates and resources under data/)"));
    if !run.status.success() {
        panic!("{tool} failed:\n{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    }
}

fn main() {
    println!("cargo:rerun-if-changed=data");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let data = Path::new("data");

    let blueprints = files(&data.join("ui"), "blp");
    let compiled = out.join("ui");
    std::fs::create_dir_all(&compiled).unwrap();
    let mut blueprint = Command::new("blueprint-compiler");
    blueprint.arg("batch-compile").arg(&compiled).arg(data.join("ui")).args(&blueprints);
    run(blueprint);

    let mut entries = String::new();
    for blp in &blueprints {
        let name = blp.file_stem().unwrap().to_string_lossy();
        // AdwApplication opens it from the base path for app.shortcuts.
        let alias = if name == "shortcuts-dialog" { " alias=\"shortcuts-dialog.ui\"" } else { "" };
        entries += &format!("    <file preprocess=\"xml-stripblanks\"{alias}>ui/{name}.ui</file>\n");
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
    // Not glib_build_tools::compile_resources: it has cargo watch the files written above, which reruns this script on every build.
    let mut resources = Command::new("glib-compile-resources");
    resources.arg("--sourcedir").arg(&out).arg("--sourcedir").arg(data).arg("--target").arg(out.join("universe-desktop.gresource")).arg(&manifest);
    run(resources);
}
