// Embarque le manifeste Windows : compatibilité Windows 8+ (obligatoire pour une
// fenêtre enfant transparente pixel par pixel) et DPI par écran.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = std::path::Path::new(&dir).join("pokagent.manifest");
    println!("cargo:rerun-if-changed=pokagent.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
    }
}
