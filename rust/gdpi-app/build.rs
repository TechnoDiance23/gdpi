//! Windows hedefinde: yönetici isteyen manifest + kalkan simgesi exe'ye gömülür.
#[path = "src/shield.rs"]
#[allow(dead_code)]
mod shield;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/shield.rs");
    println!("cargo:rerun-if-changed=assets/windows/gdpi.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out.join("gdpi.ico");
    std::fs::write(&ico, shield::shield_ico(true, &[16, 24, 32, 48, 256])).unwrap();
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/windows/gdpi.manifest");
    let esc = |p: &std::path::Path| p.display().to_string().replace('\\', "\\\\");
    let rc = out.join("gdpi.rc");
    std::fs::write(
        &rc,
        format!("1 ICON \"{}\"\n1 24 \"{}\"\n", esc(&ico), esc(&manifest)),
    )
    .unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_required().unwrap();
}
