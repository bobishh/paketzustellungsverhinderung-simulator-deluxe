//! Browser MetaMesh transport. JS owns the async Iroh endpoint, Rust owns game facts.
use crate::{Fact, Origin};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(
    inline_js = "export async function streetConnect(origin,invite){return await globalThis.paketMesh.connect(JSON.parse(origin),invite);} export function streetPublish(id,fact){globalThis.paketMesh.publish(id,JSON.parse(fact));} export function streetPoll(id){return JSON.stringify(globalThis.paketMesh.poll(id));} export function streetClose(id){globalThis.paketMesh.close(id);}"
)]
extern "C" {
    #[wasm_bindgen(catch, js_name = streetConnect)]
    async fn connect(origin: &str, invite: Option<String>) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = streetPublish)]
    fn publish(id: &str, fact: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = streetPoll)]
    fn poll(id: &str) -> String;
    #[wasm_bindgen(js_name = streetClose)]
    fn close(id: &str);
}
#[derive(Deserialize)]
pub struct SharedStreet {
    pub origin: Origin,
    pub device: String,
    pub invitation: String,
    pub is_host: bool,
}
pub async fn connect_browser(
    origin: Origin,
    invite: Option<String>,
) -> Result<SharedStreet, String> {
    let result = connect(&serde_json::to_string(&origin).unwrap(), invite)
        .await
        .map_err(|e| format!("{e:?}"))?;
    serde_json::from_str(&result.as_string().ok_or("invalid session response")?)
        .map_err(|e| e.to_string())
}
impl SharedStreet {
    pub fn publish(&self, fact: Fact) -> Result<(), String> {
        if !fact.origin().matches(self.origin) {
            return Err("fact origin does not match session".into());
        }
        publish(
            &self.device,
            &serde_json::to_string(&fact).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{e:?}"))
    }
    pub fn poll(&self) -> Vec<Fact> {
        serde_json::from_str::<Vec<Fact>>(&poll(&self.device))
            .unwrap_or_default()
            .into_iter()
            .filter(|fact| fact.origin().matches(self.origin))
            .collect()
    }
}
impl Drop for SharedStreet {
    fn drop(&mut self) {
        close(&self.device);
    }
}
pub fn invite_link(invitation: &str) -> String {
    // Browser invitations are complete HTTPS links and also work from chat messages.
    format!(
        "https://paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev/#join={}",
        URL_SAFE_NO_PAD.encode(invitation)
    )
}
pub fn invite_origin(input: &str) -> Result<Origin, String> {
    let encoded = input.trim().split_once("#join=").map(|(_, value)| value);
    let bytes = match encoded {
        Some(value) => URL_SAFE_NO_PAD.decode(value).map_err(|e| e.to_string())?,
        None => input.as_bytes().to_vec(),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["protocol"] != "paket-web/1" {
        return Err("Use a browser ride invitation".into());
    }
    let origin: Origin =
        serde_json::from_value(value["origin"].clone()).map_err(|e| e.to_string())?;
    if !origin.lat.is_finite()
        || origin.lat.abs() > 85.
        || !origin.lon.is_finite()
        || origin.lon.abs() > 180.
    {
        return Err("invalid invite location".into());
    }
    Ok(origin)
}
pub struct AvailableRide {
    pub origin: Origin,
    pub link: String,
}
pub struct Discovery;
impl Discovery {
    pub fn listen() -> Result<Self, String> {
        Err("Browser rides join by invitation".into())
    }
    pub fn poll(&mut self) -> Vec<(&str, &AvailableRide)> {
        vec![]
    }
}
