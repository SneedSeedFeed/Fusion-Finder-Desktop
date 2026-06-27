use std::{path::PathBuf, sync::RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favourite {
    pub head_dex: u16,
    pub body_dex: u16,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Favourites {
    fusions: Vec<Favourite>,
}

impl Favourites {
    pub fn list(&self) -> &[Favourite] {
        &self.fusions
    }

    pub fn toggle(&mut self, fav: Favourite) -> bool {
        if let Some(pos) = self.fusions.iter().position(|f| *f == fav) {
            self.fusions.remove(pos);
            false
        } else {
            self.fusions.push(fav);
            true
        }
    }
}

#[derive(Default)]
pub struct FavouritesState(pub RwLock<Favourites>);

fn favourites_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("favourites.json"))
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Favourites {
    let Some(path) = favourites_path(app) else {
        return Favourites::default();
    };
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save<R: Runtime>(app: &AppHandle<R>, favourites: &Favourites) {
    let Some(path) = favourites_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match serde_json::to_vec_pretty(favourites) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&path, bytes) {
                eprintln!("could not save favourites: {e}");
            }
        }
        Err(e) => eprintln!("could not serialize favourites: {e}"),
    }
}
