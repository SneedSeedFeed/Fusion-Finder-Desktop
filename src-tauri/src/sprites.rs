//! Fusion sprite resolution + caching.
//!
//! Infinite Fusion stores a sprite for fusion `head.body` as a cell in a per-*head* spritesheet
//! (96px cells, indexed by the body's in-game dex number). There are two tiers:
//!
//! * **custom** — hand-drawn, 20-column sheets fetched from the server and cached to disk. Which
//!   fusions have one is declared by `Data/sprites/CUSTOM_SPRITES` (the [`SpriteManifest`]).
//! * **autogen** — procedural, 10-column sheets that ship locally with the game.
//!
//! Resolution per request: if the manifest declares a custom *and* the fetched cell isn't blank,
//! serve it; otherwise fall back to autogen; and if there's no autogen either (e.g. the Gen-3
//! species whose ids outrun the local autogen sheets), a transparent placeholder. The manifest
//! lies sometimes (declares a custom that is actually an empty cell), which is why the blank-cell
//! check drives the fallback rather than trusting the manifest outright.
//!
//! Network fetches of custom sheets are rate-limited to match the games' own limiter (15 / 60s,
//! 5 concurrent). Over budget a fetch *waits* for a slot rather than dropping, so a custom sprite
//! still loads (after a short delay) without a refresh; fetched sheets and 404s are both cached.

use std::{
    collections::{HashSet, VecDeque},
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use image::{ImageFormat, RgbaImage};
use moka::future::Cache;
use snafu::{OptionExt, ResultExt, Snafu};

const SPRITE_SIZE: u32 = 96;
const CUSTOM_COLUMNS: u32 = 20;
const AUTOGEN_COLUMNS: u32 = 10;
const CUSTOM_SHEET_URL_BASE: &str =
    "https://infinitefusion.net/customsprites/spritesheets/spritesheets_custom";

// Mirror the games' own custom-sprite download limiter (CUSTOMSPRITES_RATE_* in their downloaded
// `Settings.rb`): at most 15 fetches in any rolling 60s. Over budget we skip the fetch and fall
// back to autogen, same as the game returning `nil` from `download_spritesheet`.
const CUSTOM_FETCH_MAX: usize = 15;
const CUSTOM_FETCH_WINDOW: Duration = Duration::from_secs(60);
// And never more than this many fetches in flight at once (the game's MAX_NB_SPRITES_TO_DOWNLOAD_AT_ONCE).
const MAX_CONCURRENT_FETCHES: usize = 5;

/// Finished PNG bytes for a single fused sprite, shared across cache hits.
pub type SpriteBytes = Arc<[u8]>;

/// Which fusions the game claims have a hand-drawn custom sprite, parsed from
/// `Data/sprites/CUSTOM_SPRITES` (lines like `1.100.png`, `1.100a.png`). We keep only the *base*
/// (letter-less) entries — that's what the default per-head spritesheet packs; alt variants live
/// in separate sheets we don't fetch yet.
#[derive(Debug, Default)]
pub struct SpriteManifest {
    base: HashSet<(u16, u16)>,
}

impl SpriteManifest {
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    fn parse(text: &str) -> Self {
        let mut base = HashSet::new();
        for line in text.lines() {
            let stem = line.trim().strip_suffix(".png").unwrap_or(line.trim());
            // `head.body`; alt entries (`1.100a`) fail the body parse and are skipped.
            if let Some((head, body)) = stem.split_once('.')
                && let (Ok(head), Ok(body)) = (head.parse::<u16>(), body.parse::<u16>())
            {
                base.insert((head, body));
            }
        }
        Self { base }
    }

    pub fn has_custom(&self, head: u16, body: u16) -> bool {
        self.base.contains(&(head, body))
    }

    pub fn len(&self) -> usize {
        self.base.len()
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }
}

#[derive(Debug, Snafu)]
pub enum SpriteError {
    #[snafu(display("no sprite available for {head}.{body}"))]
    NoSprite { head: u16, body: u16 },
    #[snafu(display("reading {path}"))]
    ReadFile {
        source: std::io::Error,
        path: String,
    },
    #[snafu(display("fetching {url}"))]
    Fetch { source: reqwest::Error, url: String },
    #[snafu(display("decoding/encoding image"))]
    Image { source: image::ImageError },
    // flattens an `Arc<SpriteError>` bubbled up from an inner (coalesced) cache load
    #[snafu(display("loading spritesheet: {detail}"))]
    Sheet { detail: String },
}

/// Sliding-window limiter mirroring the game's `requestRateExceeded?`: at most `max` requests in
/// any rolling `window`.
#[derive(Debug)]
struct RateLimiter {
    max: usize,
    window: Duration,
    hits: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(VecDeque::new()),
        }
    }

    /// Prune expired hits; if under budget record one and return `Ok`, else return how long until
    /// the oldest hit leaves the window (i.e. how long to wait for a free slot).
    fn poll(&self) -> Result<(), Duration> {
        let now = Instant::now();
        let mut hits = self.hits.lock().unwrap();
        while hits
            .front()
            .is_some_and(|&t| now.duration_since(t) > self.window)
        {
            hits.pop_front();
        }
        if hits.len() < self.max {
            hits.push_back(now);
            return Ok(());
        }
        Err((*hits.front().unwrap() + self.window).saturating_duration_since(now))
    }

    #[cfg(test)]
    fn try_acquire(&self) -> bool {
        self.poll().is_ok()
    }

    /// Wait (yielding) until a slot is free, then take it. Never exceeds `max` per `window`.
    async fn acquire(&self) {
        while let Err(wait) = self.poll() {
            tokio::time::sleep(wait.max(Duration::from_millis(20))).await;
        }
    }
}

/// Resolves + caches fusion sprites. `head`/`body` everywhere are *in-game dex numbers*
/// (`SpeciesDetails::id_number`), not our internal `SpeciesId` indices — the caller maps.
pub struct SpriteService {
    manifest: Arc<SpriteManifest>,
    /// `{game}/Graphics/Battlers/spritesheets_autogen`
    autogen_dir: PathBuf,
    /// app-managed dir we download + persist custom head sheets into
    custom_cache_dir: PathBuf,
    client: reqwest::Client,
    rate_limiter: RateLimiter,
    /// caps concurrent network fetches to `MAX_CONCURRENT_FETCHES`
    fetch_slots: tokio::sync::Semaphore,
    /// 1×1 transparent PNG served when a fusion has no sprite of any kind
    blank: SpriteBytes,
    /// decoded head sheets (big, ~20MB each) — bounded by total bytes, not count. `None` is a
    /// cached negative result (the head has no custom sheet on the server)
    custom_sheets: Cache<u16, Option<Arc<RgbaImage>>>,
    autogen_sheets: Cache<u16, Arc<RgbaImage>>,
    /// finished per-fusion PNG bytes (small) — bounded by count
    sprites: Cache<(u16, u16), SpriteBytes>,
}

impl SpriteService {
    pub fn new(game_dir: &Path, custom_cache_dir: PathBuf) -> Result<Self, SpriteError> {
        let manifest_path = game_dir.join("Data/sprites/CUSTOM_SPRITES");
        let manifest = SpriteManifest::from_file(&manifest_path).context(ReadFileSnafu {
            path: manifest_path.display().to_string(),
        })?;
        std::fs::create_dir_all(&custom_cache_dir).ok();

        let client = reqwest::Client::builder()
            .user_agent(concat!("fusion-finder/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        // weigh sheets by their raw pixel-buffer size so the cache caps memory, not entry count
        let by_bytes =
            |_k: &u16, v: &Arc<RgbaImage>| v.as_raw().len().min(u32::MAX as usize) as u32;
        let by_bytes_opt = |_k: &u16, v: &Option<Arc<RgbaImage>>| {
            v.as_ref()
                .map_or(0, |img| img.as_raw().len().min(u32::MAX as usize) as u32)
        };

        // a 1×1 transparent png; <img> stretches it over the tile's background
        let blank = encode_png(&RgbaImage::new(1, 1)).expect("encode 1x1 png");

        Ok(Self {
            manifest: Arc::new(manifest),
            autogen_dir: game_dir.join("Graphics/Battlers/spritesheets_autogen"),
            custom_cache_dir,
            client,
            rate_limiter: RateLimiter::new(CUSTOM_FETCH_MAX, CUSTOM_FETCH_WINDOW),
            fetch_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_FETCHES),
            blank,
            custom_sheets: Cache::builder()
                .weigher(by_bytes_opt)
                .max_capacity(96 * 1024 * 1024)
                .build(),
            autogen_sheets: Cache::builder()
                .weigher(by_bytes)
                .max_capacity(32 * 1024 * 1024)
                .build(),
            sprites: Cache::builder().max_capacity(8192).build(),
        })
    }

    pub fn manifest(&self) -> &SpriteManifest {
        &self.manifest
    }

    /// PNG bytes for fusion `head.body`. Always succeeds — a fusion with no custom and no autogen
    /// (e.g. the Gen-3 species the local sheets predate) resolves to a transparent placeholder.
    pub async fn get_sprite(&self, head: u16, body: u16) -> SpriteBytes {
        if let Some(bytes) = self.sprites.get(&(head, body)).await {
            return bytes;
        }
        let (bytes, cacheable) = self.resolve(head, body).await;
        // Only cache a *settled* result. A rate-limited/offline custom fetch is served as a
        // temporary autogen/blank fallback that must NOT stick, or the tile would be pinned to it
        // until restart; leaving it uncached lets the next request retry the custom.
        if cacheable {
            self.sprites.insert((head, body), bytes.clone()).await;
        }
        bytes
    }

    /// Bytes plus whether the result is settled (safe to cache). Prefers the custom sprite when
    /// the manifest declares it and the fetched cell has pixels; else autogen; else a transparent
    /// placeholder. Only an *offline/transient* custom-fetch failure is uncacheable (so it retries);
    /// a confirmed-missing sheet (404) or blank cell settles to autogen.
    async fn resolve(&self, head: u16, body: u16) -> (SpriteBytes, bool) {
        if self.manifest.has_custom(head, body) {
            match self.custom_sheet(head).await {
                // sheet present: use the cell if it has pixels, else fall through to autogen
                Ok(Some(sheet)) => {
                    if let Some(cell) = crop_cell(&sheet, body, CUSTOM_COLUMNS)
                        && !is_blank(&cell)
                        && let Ok(bytes) = encode_png(&cell)
                    {
                        return (bytes, true);
                    }
                }
                // confirmed no custom sheet for this head (404) -> autogen settles it
                Ok(None) => {}
                Err(e) => {
                    eprintln!("custom sheet {head}: {e}");
                    return (self.autogen_or_blank(head, body).await, false);
                }
            }
        }
        (self.autogen_or_blank(head, body).await, true)
    }

    /// Autogen cell if one exists, otherwise the transparent placeholder.
    async fn autogen_or_blank(&self, head: u16, body: u16) -> SpriteBytes {
        self.autogen_cell(head, body)
            .await
            .unwrap_or_else(|_| self.blank.clone())
    }

    async fn autogen_cell(&self, head: u16, body: u16) -> Result<SpriteBytes, SpriteError> {
        let sheet = self
            .autogen_sheet(head)
            .await
            .map_err(|e| SpriteError::Sheet {
                detail: e.to_string(),
            })?;
        let cell =
            crop_cell(&sheet, body, AUTOGEN_COLUMNS).context(NoSpriteSnafu { head, body })?;
        if is_blank(&cell) {
            return NoSpriteSnafu { head, body }.fail();
        }
        encode_png(&cell)
    }

    /// `Ok(Some)` = sheet decoded; `Ok(None)` = the server has no sheet for this head (404, cached
    /// so we don't keep asking); `Err` = transient (offline etc.), not cached so it retries.
    async fn custom_sheet(&self, head: u16) -> Result<Option<Arc<RgbaImage>>, Arc<SpriteError>> {
        self.custom_sheets
            .try_get_with(head, self.load_custom_sheet(head))
            .await
    }

    async fn load_custom_sheet(&self, head: u16) -> Result<Option<Arc<RgbaImage>>, SpriteError> {
        let path = self.custom_cache_dir.join(format!("{head}/{head}.png"));
        if path.is_file() {
            let bytes = tokio::fs::read(&path).await.context(ReadFileSnafu {
                path: path.display().to_string(),
            })?;
            return decode_sheet(bytes).await.map(Some);
        }

        // On disk is free; a network fetch waits for a rate-limit slot (never exceeding the game's
        // 15/60s) and a concurrency slot (the game's MAX_NB_SPRITES_TO_DOWNLOAD_AT_ONCE).
        self.rate_limiter.acquire().await;
        let _slot = self
            .fetch_slots
            .acquire()
            .await
            .expect("fetch semaphore never closed");

        let url = format!("{CUSTOM_SHEET_URL_BASE}/{head}/{head}.png");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context(FetchSnafu { url: url.clone() })?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None); // head simply has no custom sheet
        }
        let bytes = response
            .error_for_status()
            .context(FetchSnafu { url: url.clone() })?
            .bytes()
            .await
            .context(FetchSnafu { url })?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&path, &bytes).await.ok();

        decode_sheet(bytes.to_vec()).await.map(Some)
    }

    async fn autogen_sheet(&self, head: u16) -> Result<Arc<RgbaImage>, Arc<SpriteError>> {
        self.autogen_sheets
            .try_get_with(head, async {
                let path = self.autogen_dir.join(format!("{head}.png"));
                let bytes = tokio::fs::read(&path).await.context(ReadFileSnafu {
                    path: path.display().to_string(),
                })?;
                decode_sheet(bytes).await
            })
            .await
    }
}

/// Crop the 96×96 cell for `body` out of a `columns`-wide sheet; `None` if it falls outside.
fn crop_cell(sheet: &RgbaImage, body: u16, columns: u32) -> Option<RgbaImage> {
    let body = u32::from(body);
    let x = (body % columns) * SPRITE_SIZE;
    let y = (body / columns) * SPRITE_SIZE;
    if x + SPRITE_SIZE > sheet.width() || y + SPRITE_SIZE > sheet.height() {
        return None;
    }
    Some(image::imageops::crop_imm(sheet, x, y, SPRITE_SIZE, SPRITE_SIZE).to_image())
}

fn is_blank(cell: &RgbaImage) -> bool {
    cell.pixels().all(|p| p[3] == 0)
}

fn encode_png(img: &RgbaImage) -> Result<SpriteBytes, SpriteError> {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .context(ImageSnafu)?;
    Ok(Arc::from(buf.into_boxed_slice()))
}

/// Decode a full spritesheet PNG off the async runtime (these are multi-MB images).
async fn decode_sheet(bytes: Vec<u8>) -> Result<Arc<RgbaImage>, SpriteError> {
    tokio::task::spawn_blocking(move || {
        image::load_from_memory_with_format(&bytes, ImageFormat::Png).map(|img| img.into_rgba8())
    })
    .await
    .expect("sheet decode task panicked")
    .map(Arc::new)
    .context(ImageSnafu)
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::{RateLimiter, SpriteManifest, SpriteService};

    #[test]
    fn rate_limiter_caps_then_frees() {
        let rl = RateLimiter::new(2, Duration::from_millis(60));
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(
            !rl.try_acquire(),
            "3rd request within the window is blocked"
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(rl.try_acquire(), "a slot frees once the window passes");
    }

    #[test]
    fn manifest_parses_base_skips_alts() {
        let m = SpriteManifest::parse("1.4.png\n1.4a.png\n10.250.png\nbogus\n5.\n");
        assert!(m.has_custom(1, 4));
        assert!(m.has_custom(10, 250));
        assert!(!m.has_custom(1, 5));
        // `1.4a` (alt) and the malformed lines don't add base entries
        assert_eq!(m.len(), 2);
    }

    /// End-to-end over the *local* autogen tier (no network): loads the real game's manifest +
    /// autogen sheet, slices a cell, and checks it round-trips to a 96×96 PNG. Exercises the
    /// 10-column crop math + blank detection + encode.
    #[tokio::test]
    async fn autogen_cell_round_trips() {
        let dir = crate::test::infinite_fusion_dir();
        let cache = std::env::temp_dir().join("ff-sprite-test-cache");
        let service = SpriteService::new(&dir, cache).unwrap();

        // the manifest should have loaded thousands of declared customs
        assert!(service.manifest().len() > 1000);

        // bulbasaur(1) + charmander(4): autogen exists for every valid pair
        let bytes = service.autogen_cell(1, 4).await.unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (96, 96));
    }
}
