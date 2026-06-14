pub mod infinite_fusion;
pub mod macros;

use tauri::State;

use crate::infinite_fusion::{
    GameVersion, InfiniteFusionDex,
    filters::{FilterOptions, Filters},
};

#[tauri::command]
fn bootstrap(dex: State<'_, InfiniteFusionDex>) -> FilterOptions {
    dex.filter_options()
}

/// Run a filter set, returning matching fusion ids (`head * species_count + body`).
#[tauri::command]
fn search(dex: State<'_, InfiniteFusionDex>, filters: Filters) -> Vec<u32> {
    filters.apply(dex.inner()).iter().collect()
}

fn load_game() -> Result<InfiniteFusionDex, String> {
    // TODO: proper game-directory selection. For now read INFINITE_FUSION_DIR (set in dev via
    // .cargo/config.toml); set it in the run environment otherwise.
    let dir = std::env::var("INFINITE_FUSION_DIR")
        .map_err(|_| "INFINITE_FUSION_DIR is not set".to_string())?;
    InfiniteFusionDex::from_path(dir, GameVersion::Kanto).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // the app still starts if loading fails the commands just reject until a game is loaded.
    match load_game() {
        Ok(dex) => builder = builder.manage(dex),
        Err(e) => eprintln!("could not load game data: {e}"),
    }

    builder
        .invoke_handler(tauri::generate_handler![bootstrap, search])
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
