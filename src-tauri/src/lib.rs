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

    // key is in Data/Scripts/001_Technical/000_Encryption.rb
    // hoenn encrypts their data for some reason
    pub(crate) fn maybe_decrypt(mut bytes: Vec<u8>) -> Vec<u8> {
        const KEY: [u8; 16] = [
            0x4A, 0x8F, 0x2C, 0xE1, 0x73, 0xB5, 0x96, 0x0D, 0x5E, 0xA2, 0x3F, 0xC7, 0x81, 0x14,
            0x6B, 0xD9,
        ];

        if !bytes.starts_with(&[0x04, 0x08]) {
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte ^= KEY[i % KEY.len()];
            }
        }

        bytes
    }
}
