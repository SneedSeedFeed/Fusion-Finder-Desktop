pub mod favourites;
pub mod infinite_fusion;
pub mod macros;
pub mod sprites;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::{Serializer, ser::SerializeSeq};
use tauri::{
    AppHandle, Manager, Runtime, State,
    http::{Response, Uri},
};

use crate::favourites::{Favourite, FavouritesState};
use crate::infinite_fusion::{
    GameVersion, InfiniteFusionDex,
    area::AreaEncounter,
    bootstrap::Bootstrap,
    filters::{Filters, Metric, StatMask, order_matches},
    inspect::{FusionDetail, FusionName},
    move_card::MoveCard,
    moves::MoveId,
    species::{SpeciesId, base_stats::Stat, name_halves::NameMap},
    types::TypeId,
};
use crate::sprites::SpriteService;

/// Stored app config (where's the game what is it)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct GameConfig {
    dir: PathBuf,
    /// game version, derived from NameMap::relative_path since that's unique per game
    version: GameVersion,
}

/// dex plus its sprite service and the config that produced them
#[derive(Debug)]
struct Loaded {
    dex: InfiniteFusionDex,
    // Arc'd so it can be held across await boundaries instead of a lock
    sprites: Arc<SpriteService>,
    config: GameConfig,
}

/// State None means we should be or are on the splash screen
#[derive(Default)]
struct AppState(RwLock<Option<Loaded>>);

// todo! replace string errors?

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, String> {
    let guard = state.0.read().unwrap();
    let loaded = guard.as_ref().ok_or("no game loaded")?;
    Ok(loaded.dex.bootstrap())
}

/// Rich detail for one fusion (head/body are our species indices), including its custom sprite
/// variants + attribution.
#[tauri::command]
fn fusion_detail(
    state: State<'_, AppState>,
    head: SpeciesId,
    body: SpeciesId,
) -> Result<FusionDetail, String> {
    let guard = state.0.read().unwrap();
    let loaded = guard.as_ref().ok_or("no game loaded")?;
    let mut detail = loaded.dex.fusion_detail(head, body);
    detail.sprites = loaded
        .sprites
        .variants(detail.head.dex_id, detail.body.dex_id)
        .into_iter()
        .map(|(variant, artist)| crate::infinite_fusion::inspect::SpriteVariant { variant, artist })
        .collect();
    Ok(detail)
}

/// Run a filter set, returning the matching fusion ids (`head * species_count + body`) ordered
/// ascending by `metric`, or by the `metric / metric2` ratio when both are given. `synergy` is the
/// set of stats that count toward the synergy metrics (empty / absent = all of them).
#[tauri::command]
fn search(
    state: State<'_, AppState>,
    filters: Filters,
    metric: Option<Metric>,
    metric2: Option<Metric>,
    synergy: Option<Vec<Stat>>,
) -> Result<tauri::ipc::Response, &'static str> {
    let synergy_stats = synergy
        .map(|stats| StatMask::from_stats(&stats))
        .unwrap_or(StatMask::ALL);
    let guard = state.0.read().unwrap();
    let dex = &guard.as_ref().ok_or("no game loaded")?.dex;

    let order = order_matches(dex, filters.apply(dex), metric, metric2, synergy_stats);

    // pack the ids as raw little-/native-endian bytes so the IPC skips JSON entirely
    let bytes: Vec<u8> = order.iter().flat_map(|id| id.to_ne_bytes()).collect();
    Ok(tauri::ipc::Response::new(bytes))
}

/// The display name + type ids for a fusion, keyed by its encoded id so the caller can match results back to the grid cells that asked, even if the window has since scrolled.
#[derive(serde::Serialize)]
struct FusionCard {
    id: u32,
    name: FusionName,
    // manual impl so we can skip secondary type serializing
    #[serde(serialize_with = "serialize_types")]
    types: (TypeId, Option<TypeId>),
}

fn serialize_types<S: Serializer>(
    (type1, type2): &(TypeId, Option<TypeId>),
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match type2.as_ref() {
        Some(type2) => {
            let mut seq = serializer.serialize_seq(Some(2))?;
            seq.serialize_element(type1)?;
            seq.serialize_element(type2)?;
            seq.end()
        }
        None => {
            let mut seq = serializer.serialize_seq(Some(1))?;
            seq.serialize_element(type1)?;
            seq.end()
        }
    }
}

/// Hydrate the on-screen slice of a search result: the grid sends the ids currently in (or near)
/// the viewport and gets back each one's name and types
#[tauri::command]
fn fusion_cards(state: State<'_, AppState>, ids: Vec<u32>) -> Result<Vec<FusionCard>, String> {
    let guard = state.0.read().unwrap();
    let dex = &guard.as_ref().ok_or("no game loaded")?.dex;
    Ok(ids
        .into_iter()
        .map(|id| FusionCard {
            id,
            name: dex.fusion_name(id),
            types: dex.fusion_type_ids(id),
        })
        .collect())
}

/// Every distinct encounter location in the loaded game, for the area picker.
#[tauri::command]
fn area_locations(state: State<'_, AppState>) -> Result<Box<[Arc<str>]>, String> {
    let guard = state.0.read().unwrap();
    let dex = &guard.as_ref().ok_or("no game loaded")?.dex;
    Ok(dex.locations())
}

/// Every Pokémon found at `location`
#[tauri::command]
fn area_encounters(
    state: State<'_, AppState>,
    location: String,
) -> Result<Box<[AreaEncounter]>, &'static str> {
    let guard = state.0.read().unwrap();
    let dex = &guard.as_ref().ok_or("no game loaded")?.dex;
    Ok(dex.area_encounters(&location))
}

/// The hover-card for one move
#[tauri::command]
fn move_card(state: State<'_, AppState>, move_id: MoveId) -> Result<MoveCard, &'static str> {
    let guard = state.0.read().unwrap();
    let dex = &guard.as_ref().ok_or("no game loaded")?.dex;
    Ok(dex.move_card(move_id))
}

// i hate this but it works
/// Auto-detect which Infinite Fusion version lives in `dir` by checking for each version's
/// split-names script. `None` if it doesn't look like a game folder.
#[tauri::command]
fn detect_game(dir: String) -> Option<GameVersion> {
    let dir = Path::new(&dir);
    if dir.join(NameMap::relative_path()).is_file() {
        Some(GameVersion::Kanto)
    } else if dir.join(NameMap::relative_path_hoenn()).is_file() {
        Some(GameVersion::Hoenn)
    } else {
        None
    }
}

/// The currently-loaded game's config, or `None` if setup is still needed.
#[tauri::command]
fn current_game(state: State<'_, AppState>) -> Option<GameConfig> {
    state.0.read().unwrap().as_ref().map(|l| l.config.clone())
}

/// Load a game from `dir` at `version`, replacing any currently-loaded game and persisting so the next launch skips setup.
#[tauri::command]
fn load_game(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: String,
    version: GameVersion,
) -> Result<GameConfig, String> {
    let config = GameConfig {
        dir: PathBuf::from(dir),
        version,
    };
    let loaded = load(config.clone())?;
    *state.0.write().unwrap() = Some(loaded);
    save_config(&app, &config);
    Ok(config)
}

/// The user's favourited fusions, by head/body
#[tauri::command]
fn favourites(state: State<'_, FavouritesState>) -> Box<[Favourite]> {
    state.0.read().unwrap().list().iter().copied().collect()
}

/// Flip whether head,body is a favourite, returns the new state.
#[tauri::command]
fn toggle_favourite(
    app: AppHandle,
    state: State<'_, FavouritesState>,
    head_dex: u16,
    body_dex: u16,
) -> bool {
    let mut guard = state.0.write().unwrap();
    let now_favourite = guard.toggle(Favourite { head_dex, body_dex });
    favourites::save(&app, &guard);
    now_favourite
}

/// Load a `Loaded` (dex + sprites) for a config, mapping every failure to a string.
/// The sprite service caches custom sheets into the game's own sprite folder (see `SpriteService::new`).
fn load(config: GameConfig) -> Result<Loaded, String> {
    let dex =
        InfiniteFusionDex::from_path(&config.dir, config.version).map_err(|e| e.to_string())?;
    let sprites = Arc::new(SpriteService::new(&config.dir).map_err(|e| e.to_string())?);
    Ok(Loaded {
        dex,
        sprites,
        config,
    })
}

/// Path of the persisted game-selection file (`config.json` in the app config dir).
fn config_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("config.json"))
}

fn save_config<R: Runtime>(app: &AppHandle<R>, config: &GameConfig) {
    let Some(path) = config_path(app) else { return };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match serde_json::to_vec_pretty(config) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&path, bytes) {
                eprintln!("could not save game config: {e}");
            }
        }
        Err(e) => eprintln!("could not serialize game config: {e}"),
    }
}

fn load_config<R: Runtime>(app: &AppHandle<R>) -> Option<GameConfig> {
    let path = config_path(app)?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Serves fusion sprites at `fusionsprite://localhost/{head}.{body}.png`, where head/body are
/// in-game dex numbers (the `dex_id` the front end gets from `bootstrap`). Build the URL with
/// `convertFileSrc(\`${head}.${body}.png\`, "fusionsprite")`.
async fn serve_sprite<R: Runtime>(app: &AppHandle<R>, uri: &Uri) -> Response<Vec<u8>> {
    fn reply(status: u16, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
        Response::builder()
            .status(status)
            .header("Content-Type", content_type)
            .body(body)
            .unwrap()
    }

    let stem = uri.path().trim_start_matches('/');
    let stem = stem.strip_suffix(".png").unwrap_or(stem);
    let parsed = stem.split_once('.').and_then(|(h, rest)| {
        let head = h.parse::<u16>().ok()?;
        let split = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let (body, variant) = rest.split_at(split);
        Some((head, body.parse::<u16>().ok()?, variant))
    });
    let Some((head, body, variant)) = parsed else {
        return reply(
            400,
            "text/plain",
            b"expected {head}.{body}{variant}.png".to_vec(),
        );
    };

    // clone so we never hold the read guard across the await
    let service = {
        let guard = app.state::<AppState>();
        let loaded = guard.0.read().unwrap();
        match loaded.as_ref() {
            Some(l) => l.sprites.clone(),
            None => return reply(503, "text/plain", b"sprites not ready".to_vec()),
        }
    };

    let bytes = service.get_sprite(head, body, variant).await;
    Response::builder()
        .status(200)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "max-age=31536000, immutable")
        .body(bytes.to_vec())
        .unwrap()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(FavouritesState::default())
        .register_asynchronous_uri_scheme_protocol("fusionsprite", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_sprite(&app, request.uri()).await);
            });
        })
        .setup(|app| {
            // favourites live independently of the loaded game (keyed by stable dex ids)
            *app.state::<FavouritesState>().0.write().unwrap() = favourites::load(app.handle());
            // reload the previously selected game (if any)
            if let Some(config) = load_config(app.handle()) {
                match load(config) {
                    Ok(loaded) => {
                        *app.state::<AppState>().0.write().unwrap() = Some(loaded);
                    }
                    Err(e) => eprintln!("could not load saved game: {e}"),
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            search,
            fusion_cards,
            fusion_detail,
            area_locations,
            area_encounters,
            move_card,
            detect_game,
            current_game,
            load_game,
            favourites,
            toggle_favourite
        ])
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
