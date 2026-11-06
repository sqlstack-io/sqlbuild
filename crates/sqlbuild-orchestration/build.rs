//! With the `ui` feature, embed the built web UI (`ui/build`, or `SQLBUILD_UI_DIST`) for `sqb server`.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=SQLBUILD_UI_DIST");
    println!("cargo:rerun-if-env-changed=SQLBUILD_UI_REQUIRED");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("ui_assets.rs");
    let mut files = Vec::new();
    if std::env::var_os("CARGO_FEATURE_UI").is_some() {
        let dist = std::env::var_os("SQLBUILD_UI_DIST").map_or_else(
            || {
                Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest"))
                    .join("../../ui/build")
            },
            PathBuf::from,
        );
        println!("cargo:rerun-if-changed={}", dist.display());
        if dist.join("index.html").is_file() {
            collect(&dist, &dist, &mut files);
        } else {
            let message = format!(
                "sqb server will not serve the UI: {} has no index.html; run `npm ci && npm run build` in ui/ or set SQLBUILD_UI_DIST",
                dist.display()
            );
            assert!(
                std::env::var_os("SQLBUILD_UI_REQUIRED").is_none(),
                "{message}"
            );
            println!("cargo:warning={message}");
        }
    }
    files.sort();
    let entries: String = files
        .iter()
        .map(|(route, file)| {
            format!(
                "    UiAsset {{ path: {route:?}, body: include_bytes!({:?}) }},\n",
                file.display()
            )
        })
        .collect();
    std::fs::write(
        output,
        format!("pub(crate) static UI_ASSETS: &[UiAsset] = &[\n{entries}];\n"),
    )
    .expect("write the embedded UI table");
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(directory).expect("read the UI build") {
        let path = entry.expect("UI build entry").path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("inside the UI build");
            let route = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push((
                format!("/{route}"),
                path.canonicalize().expect("absolute UI asset path"),
            ));
        }
    }
}
