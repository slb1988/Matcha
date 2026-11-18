//! Embeds the Matcha application icon (`assets/icon/matcha.ico`, via `matcha.rc`) into the
//! Windows executables. On other targets `embed-resource` does nothing.

fn main() {
    println!("cargo:rerun-if-changed=matcha.rc");
    println!("cargo:rerun-if-changed=../../assets/icon/matcha.ico");
    embed_resource::compile("matcha.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to compile matcha.rc (Windows SDK rc.exe required)");
}
