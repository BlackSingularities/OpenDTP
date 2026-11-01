fn main() {
    embed_languages();
}

/// Tłumaczenia interfejsu: każdy `lang/<kod>/LC_MESSAGES/opendtp.po` (ten sam plik czyta Slint) trafia do programu.
fn embed_languages() {
    let root = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../lang");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut rs = String::from("/// Wbudowane tłumaczenia: (kod, treść pliku .po).\npub static LANGS: &[(&str, &str)] = &[\n");
    let mut dirs: Vec<_> = std::fs::read_dir(&root).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    dirs.sort();
    for dir in dirs {
        let po = dir.join("LC_MESSAGES").join("opendtp.po");
        if !po.is_file() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", po.display());
        let code = dir.file_name().unwrap().to_string_lossy().to_string();
        rs.push_str(&format!("    ({code:?}, include_str!({:?})),\n", po.display().to_string()));
    }
    rs.push_str("];\n");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("langs.rs"), rs).expect("langs.rs");
}
