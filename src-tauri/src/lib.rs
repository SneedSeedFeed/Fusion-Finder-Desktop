pub mod infinite_fusion;
pub mod macros;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
pub(crate) mod test {
    use std::path::PathBuf;

    pub(crate) fn infinite_fusion_dir() -> PathBuf {
        std::env::var("INFINITE_FUSION_DIR")
            .map(PathBuf::from)
            .unwrap()
    }

    pub(crate) fn infinite_fusion_hoenn_dir() -> PathBuf {
        std::env::var("INFINITE_FUSION_HOENN_DIR")
            .map(PathBuf::from)
            .unwrap()
    }

    pub(crate) fn maybe_decrypt(bytes: Vec<u8>) -> Vec<u8> {
        crate::infinite_fusion::maybe_decrypt(bytes)
    }
}
