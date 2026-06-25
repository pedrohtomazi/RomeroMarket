fn main() {
    println!("cargo:rerun-if-env-changed=NPCAP_SDK_LIB");

    if let Some(path) = npcap_sdk_lib_path() {
        println!("cargo:rustc-link-search=native={}", path.display());
    }

    tauri_build::build()
}

fn npcap_sdk_lib_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("NPCAP_SDK_LIB") {
        let path = std::path::PathBuf::from(path);
        if path.join("wpcap.lib").exists() {
            return Some(path);
        }
    }

    [
        "C:\\Npcap-sdk\\Lib\\x64",
        "C:\\Program Files\\Npcap SDK\\Lib\\x64",
        "C:\\Program Files (x86)\\Npcap SDK\\Lib\\x64",
        "C:\\WpdPack\\Lib\\x64",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|path| path.join("wpcap.lib").exists())
}
