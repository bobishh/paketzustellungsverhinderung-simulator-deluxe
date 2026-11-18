//! Platform I/O. The game and its rules are shared by desktop and browser builds.
use paket_geo::{LatLon, Scene, TileId};
use std::{path::PathBuf, sync::mpsc::Sender};

#[cfg(not(target_arch = "wasm32"))]
pub fn arguments() -> Vec<String> {
    std::env::args().skip(1).collect()
}
#[cfg(not(target_arch = "wasm32"))]
pub fn copy_text(text: &str) -> bool {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .is_ok()
}
#[cfg(not(target_arch = "wasm32"))]
pub fn load_scene(
    cache: PathBuf,
    origin: LatLon,
    tile: TileId,
    network: bool,
    tx: Sender<Result<Scene, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(paket_geo::load_tile(&cache, origin, tile, network));
    });
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(
        inline_js = "export function gameArgs(){return JSON.stringify(globalThis.paketArgs || []);} export async function gameFetch(url){const r=await fetch(url,{signal:AbortSignal.timeout(url.startsWith('/api/osm?')?60000:url.startsWith('/elevation/')?4200:15000)}); if(!r.ok) throw new Error('Map request failed: '+r.status); const bytes=new Uint8Array(await r.arrayBuffer()); if(bytes.length>16000000)throw new Error('Map response too large'); return bytes;} export function gameCopy(text){navigator.clipboard.writeText(text).catch(()=>globalThis.paketNotify?.('Copy failed. Select the invite link to copy it.'));} export function gamePhoto(bytes,index){globalThis.paketPhoto(bytes,index);}"
    )]
    extern "C" {
        #[wasm_bindgen(js_name = gameArgs)]
        fn args() -> String;
        #[wasm_bindgen(catch, js_name = gameFetch)]
        async fn fetch(url: &str) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = gameCopy)]
        fn copy(text: &str) -> Result<(), JsValue>;
        #[wasm_bindgen(catch, js_name = gamePhoto)]
        fn photo(bytes: &[u8], index: usize) -> Result<(), JsValue>;
    }
    #[wasm_bindgen(
        inline_js = "export function gamePointerLocked(){return document.pointerLockElement?.id === 'game';} export function gameReleasePointer(){if(gamePointerLocked())document.exitPointerLock();}"
    )]
    extern "C" {
        #[wasm_bindgen(js_name = gamePointerLocked)]
        pub fn pointer_locked() -> bool;
        #[wasm_bindgen(js_name = gameReleasePointer)]
        pub fn release_pointer();
    }
    pub fn arguments() -> Vec<String> {
        serde_json::from_str(&args()).unwrap_or_default()
    }
    pub fn copy_text(text: &str) -> bool {
        copy(text).is_ok()
    }
    pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
        fetch(url)
            .await
            .map(|bytes| js_sys::Uint8Array::new(&bytes).to_vec())
            .map_err(|e| format!("{e:?}"))
    }
    pub fn load_scene(
        _: PathBuf,
        origin: LatLon,
        tile: TileId,
        _: bool,
        tx: Sender<Result<Scene, String>>,
    ) {
        wasm_bindgen_futures::spawn_local(async move {
            let url = format!(
                "/api/osm?lat={:.5}&lon={:.5}&x={}&y={}",
                origin.lat, origin.lon, tile.x, tile.y
            );
            let result = fetch_bytes(&url)
                .await
                .and_then(|bytes| paket_geo::parse_overpass(&bytes, origin));
            let _ = tx.send(result);
        });
    }
    pub fn save_photo(image: &bevy::prelude::Image, index: usize) -> bool {
        let Ok(image) = image.clone().try_into_dynamic() else {
            return false;
        };
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).is_ok()
            && photo(png.get_ref(), index).is_ok()
    }
}
#[cfg(target_arch = "wasm32")]
pub use browser::*;

pub fn ui_event(name: &str, text: &str) {
    crate::game_ui::record(name, text);
    #[cfg(not(target_arch = "wasm32"))]
    if crate::native_audio::accepts(name) {
        if let Ok(value) = serde_json::from_str(text) {
            crate::native_audio::event(name, &value);
        }
    }
    #[cfg(target_arch = "wasm32")]
    browser_ui_event(name, text);
}
pub fn ui_commands() -> String {
    let mut values = crate::game_ui::drain_commands();
    #[cfg(target_arch = "wasm32")]
    values.extend(
        serde_json::from_str::<Vec<serde_json::Value>>(&browser_commands()).unwrap_or_default(),
    );
    values.retain(|command| {
        if command["action"] == "ui-event" {
            if let Some(name) = command["name"].as_str() {
                crate::game_ui::record(name, &command["data"].to_string());
            }
            false
        } else {
            true
        }
    });
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".into())
}
pub fn text_inputs() -> Vec<serde_json::Value> {
    #[cfg(target_arch = "wasm32")]
    {
        serde_json::from_str(&browser_text_inputs()).unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Vec::new()
    }
}
pub fn service(value: &serde_json::Value) {
    #[cfg(target_arch = "wasm32")]
    browser_service(&value.to_string());
    #[cfg(not(target_arch = "wasm32"))]
    crate::game_ui::native_service(value);
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function gameUi(name,json){globalThis.dispatchEvent(new CustomEvent(name,{detail:JSON.parse(json)}));} export function gameCommands(){return JSON.stringify(globalThis.paketCommands?.splice(0,64)||[]);} export function gameService(json){globalThis.dispatchEvent(new CustomEvent('paket-service',{detail:JSON.parse(json)}));} export function gameTextInputs(){return JSON.stringify(globalThis.paketTextInputs?.splice(0,32)||[]);}"
)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = gameUi)]
    fn browser_ui_event(name: &str, json: &str);
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = gameCommands)]
    fn browser_commands() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = gameService)]
    fn browser_service(json: &str);
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = gameTextInputs)]
    fn browser_text_inputs() -> String;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render_frame(_: f32) {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function renderFrame(ms){globalThis.paketFrame?.(ms);}"
)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(ms: f32);
}
