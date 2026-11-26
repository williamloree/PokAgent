// Embarque l'icône (pokagent.rc) et le manifeste Windows : compatibilité
// Windows 8+ et DPI par écran.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = std::path::Path::new(&dir).join("pokagent.manifest");
    println!("cargo:rerun-if-changed=pokagent.manifest");
    println!("cargo:rerun-if-changed=assets/pokagent.ico");
    embed_resource::compile("pokagent.rc", embed_resource::NONE).manifest_optional().unwrap();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
    }
}
