pub mod infinite_fusion;
pub mod macros;
pub mod sprites;

use std::path::PathBuf;

use tauri::{
    AppHandle, Manager, Runtime, State,
    http::{Response, Uri},
};

use crate::infinite_fusion::{
    GameVersion, InfiniteFusionDex,
    filters::{FilterOptions, Filters, SortBy, SortOrder},
};
use crate::sprites::SpriteService;

#[tauri::command]
fn bootstrap(dex: State<'_, InfiniteFusionDex>) -> FilterOptions {
    dex.filter_options()
}

/// Run a filter set, returning matching fusion ids (`head * species_count + body`), ordered by
/// `sort` (dex order by default, else a fused stat descending).
#[tauri::command]
fn search(
    dex: State<'_, InfiniteFusionDex>,
    filters: Filters,
    sort: SortBy,
    descending: bool,
) -> Vec<u32> {
    let sort_order = if descending {
        SortOrder::Descending
    } else {
        SortOrder::Ascending
    };
    sort.order(dex.inner(), filters.apply(dex.inner()), sort_order)
}

fn game_dir() -> Result<PathBuf, String> {
    // TODO: proper game-directory selection. For now read INFINITE_FUSION_DIR (set in dev via
    // .cargo/config.toml); set it in the run environment otherwise.
    std::env::var("INFINITE_FUSION_DIR")
        .map(PathBuf::from)
        .map_err(|_| "INFINITE_FUSION_DIR is not set".to_string())
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
    let parsed = stem
        .split_once('.')
        .and_then(|(h, b)| Some((h.parse::<u16>().ok()?, b.parse::<u16>().ok()?)));
    let Some((head, body)) = parsed else {
        return reply(400, "text/plain", b"expected {head}.{body}.png".to_vec());
    };

    let Some(service) = app.try_state::<SpriteService>() else {
        return reply(503, "text/plain", b"sprites not ready".to_vec());
    };

    // always 200: a fusion with no sprite resolves to a transparent placeholder, not an error
    let bytes = service.get_sprite(head, body).await;
    Response::builder()
        .status(200)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "max-age=31536000, immutable")
        .body(bytes.to_vec())
        .unwrap()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // the app still starts if loading fails the commands just reject until a game is loaded.
    let dir = match game_dir().and_then(|dir| {
        InfiniteFusionDex::from_path(&dir, GameVersion::Kanto)
            .map(|dex| (dir, dex))
            .map_err(|e| e.to_string())
    }) {
        Ok((dir, dex)) => {
            builder = builder.manage(dex);
            Some(dir)
        }
        Err(e) => {
            eprintln!("could not load game data: {e}");
            None
        }
    };

    builder
        .register_asynchronous_uri_scheme_protocol("fusionsprite", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_sprite(&app, request.uri()).await);
            });
        })
        .setup(move |app| {
            // Sprites are optional: if the game dir or manifest is missing the app still runs,
            // the protocol just 404s.
            if let Some(dir) = &dir {
                let cache_dir = app
                    .path()
                    .app_cache_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join("spritesheets_custom");
                match SpriteService::new(dir, cache_dir) {
                    Ok(service) => {
                        app.manage(service);
                    }
                    Err(e) => eprintln!("sprite service init failed: {e}"),
                }
            }
            Ok(())
        })
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
