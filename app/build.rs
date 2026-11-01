fn main() {
    // Ikona i opis pliku .exe (Eksplorator, pasek zadań, „Aplikacje i funkcje”).
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-env-changed=OPENDTP_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::path::Path::new("icon.ico").exists() {
        let mut res = winresource::WindowsResource::new();
        let ver = std::env::var("OPENDTP_VERSION").unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap());
        res.set_icon("icon.ico")
            .set("FileDescription", "OpenDTP")
            .set("ProductName", "OpenDTP")
            .set("CompanyName", "Adam Rędzikowski")
            .set("LegalCopyright", "Adam Rędzikowski · redzikowski.com")
            .set("ProductVersion", &ver)
            .set("FileVersion", &ver);
        if let Err(e) = res.compile() {
            println!("cargo:warning=ikona .exe pominięta: {e}");
        }
    }
    // Ikony poleceń dla elementów budowanych z Rusta (pasek szybkiego dostępu, paleta, Moja karta).
    let dir = std::path::Path::new("ui/icons");
    println!("cargo:rerun-if-changed=ui/icons");
    let mut rs = String::from("pub static ICONS: &[(&str, &[u8])] = &[\n");
    let mut names: Vec<String> = std::fs::read_dir(dir).map(|d| d.flatten().filter_map(|e| e.file_name().to_str().map(String::from)).filter(|n| n.ends_with(".svg")).collect()).unwrap_or_default();
    names.sort();
    let abs = std::fs::canonicalize(dir).unwrap();
    for n in names {
        rs.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", n.trim_end_matches(".svg"), abs.join(&n).display().to_string()));
    }
    rs.push_str("];\n");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("icons.rs"), rs).unwrap();
    // Tłumaczenia: lang/<kod>/LC_MESSAGES/opendtp.po (te same pliki czyta core/src/lang.rs).
    println!("cargo:rerun-if-changed=../lang");
    let cfg = slint_build::CompilerConfiguration::new()
        .with_style("fluent-dark".into())
        .with_bundled_translations("../lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/main.slint", cfg).expect("slint");
}
