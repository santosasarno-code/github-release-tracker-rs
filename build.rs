// Embeds the icon, the Windows manifest (modern controls, DPI awareness) and the version info.
fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").map_or(false, |os| os == "windows") {
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("could not embed the resources (is the Windows SDK installed? see README)");
    }
}
