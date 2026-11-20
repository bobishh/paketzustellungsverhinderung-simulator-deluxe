//! Presentation of existing parcel state; delivery rules remain in Jobs.
use serde_json::Value;
pub(crate) fn delivery(h: &Value) -> String {
    let format = |step: &str, detail: String, action: String| format!("{step}\n{detail}\n{action}");
    if h["ready"] != true {
        return format(
            "Joining shift",
            "Waiting for the shared clock.".into(),
            "Joining the shift…".into(),
        );
    }
    if h["ended"] == true {
        return format(
            "Day complete",
            "Compare scores and vote for tomorrow’s neighbourhood.".into(),
            "Vote for the next day".into(),
        );
    }
    if h["stunSeconds"].as_f64().unwrap_or(0.) > 0. {
        return format(
            "Bitten · recovering",
            "Your parcel and KPI are safe.".into(),
            format!("OUCH! · {:.1}s", h["stunSeconds"].as_f64().unwrap_or(0.)),
        );
    }
    if h["form"] == "dog" {
        return format(
            "Cause trouble",
            "Bite nearby couriers or scratch a closed door.".into(),
            "E: bite / scratch · Space: jump · G: mark van".into(),
        );
    }
    if let Some(loaded) = h["bulk"].as_bool() {
        return format(
            "Delivery of the Day · +500 KPI",
            if loaded {
                "Leave all remaining parcels at the marked spot."
            } else {
                "Collect the remaining parcels at your van."
            }
            .into(),
            if h["form"] == "van" {
                "V · Leave van"
            } else {
                h["prompt"].as_str().unwrap_or_default()
            }
            .into(),
        );
    }
    if h["form"] == "van" {
        return format(
            "1 / 4 · Get the parcel",
            "Park at the orange marker, exit and collect the box beside your van.".into(),
            "V · Leave van, then E · Collect parcel".into(),
        );
    }
    let active = h["active"].as_u64().unwrap_or(0) as usize;
    let selected = h["selected"].as_u64().unwrap_or(0) as usize;
    let Some(parcel) = h["routes"].as_array().and_then(|r| r.get(active)) else {
        return format(
            "Finding addresses",
            "The route fills as streets load.".into(),
            "Loading your route…".into(),
        );
    };
    let state = parcel["state"].as_str().unwrap_or("");
    let at_van = h["vanDistance"].as_f64().is_some_and(|d| d <= 3.7);
    let camera = h["photo"] == true;
    if state == "carrying" && at_van && !camera && (active != selected || parcel["rung"] != true) {
        return format(
            "At your van",
            format!(
                "You are carrying the box for {}.",
                parcel["address"].as_str().unwrap_or("this stop")
            ),
            if active != selected && h["routes"][selected]["state"] == "in-van" {
                "E · Exchange for selected parcel"
            } else {
                "E · Return parcel to van"
            }
            .into(),
        );
    }
    if active != selected && matches!(state, "carrying" | "in-flight" | "photo-due") {
        return format(
            "Inspecting another stop",
            format!(
                "Your current parcel is for {}.",
                parcel["address"].as_str().unwrap_or("another stop")
            ),
            if state == "carrying" {
                "X: set aside · Q near van: return box"
            } else {
                "Tab: return to the box’s stop"
            }
            .into(),
        );
    }
    if camera && !matches!(state, "photo-due" | "in-flight") {
        return format(
            "Put the camera away",
            "Press F before handling a parcel.".into(),
            "F · Put camera away".into(),
        );
    }
    match state{
        "in-van"=>format("1 / 4 · Get the parcel","The box is still in your van.".into(),if at_van{"E · Collect this parcel".into()}else{format!("Return to your van{} · then E",h["vanDistance"].as_f64().map(|v|format!(" · {v:.0} m")).unwrap_or_default())}),
        "set-aside"=>format("Box set aside","The marker points to your box.".into(),if parcel["parcelDistance"].as_f64().is_some_and(|d|d<=3.4){"E · Pick up box"}else{"Follow the box marker"}.into()),
        "in-flight"=>format("3 / 4 · Parcel airborne","Wait for it to stop. The marker follows the box.".into(),"Wait for the parcel to land".into()),
        "carrying" if parcel["rung"]!=true=>format("2 / 4 · Ring the recipient","At the marked entrance press R, click the bell beside the recipient’s name, then Esc.".into(),"R: intercom · X: set aside".into()),
        "carrying"=>format("3 / 4 · Leave it somewhere","E places it at a doorstep. A handover to the resident earns 0 KPI.".into(),"E: place / hand over · T · Throw · X: set aside".into()),
        "photo-due"=>format("4 / 4 · Photograph the box","Centre the box within 9 m, with a clear view. Space saves proof and selects the next stop.".into(),if camera{if h["photoReady"]==true{"Space · Take delivery photo"}else{"Aim at the box · F to put camera away"}}else{"F: open camera · E nearby: pick up"}.into()),
        _=>format("Delivery complete","The next unfinished delivery is selected automatically.".into(),"Tab: inspect another stop".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn state() -> Value {
        json!({"selected":0,"active":0,"routes":[{"index":0,"address":"Example 12","state":"in-van","rung":false}],"ready":true,"ended":false,"form":"courier","photo":false,"photoReady":false,"bulk":null,"vanDistance":3,"stunSeconds":0})
    }
    #[test]
    fn next_day_uses_vote() {
        let mut h = state();
        h["ended"] = json!(true);
        let s = delivery(&h);
        assert!(s.contains("vote for tomorrow"));
        assert!(s.contains("Vote for the next day"));
    }
    #[test]
    fn van_and_collect_guidance() {
        let mut h = state();
        h["form"] = json!("van");
        assert!(delivery(&h).contains("V · Leave van"));
        h["form"] = json!("courier");
        assert!(delivery(&h).contains("E · Collect this parcel"));
        h["vanDistance"] = json!(30);
        assert!(delivery(&h).contains("Return to your van · 30 m"));
    }
    #[test]
    fn bell_precedes_parcel_throw() {
        let mut h = state();
        h["vanDistance"] = json!(30);
        h["routes"][0]["state"] = json!("carrying");
        assert!(delivery(&h).contains("click the bell"));
        h["routes"][0]["rung"] = json!(true);
        assert!(delivery(&h).contains("T · Throw"));
    }
    #[test]
    fn flight_and_camera_are_distinct() {
        let mut h = state();
        h["routes"][0]["state"] = json!("in-flight");
        assert!(delivery(&h).contains("Wait"));
        h["routes"][0]["state"] = json!("photo-due");
        h["photo"] = json!(true);
        h["photoReady"] = json!(true);
        assert!(delivery(&h).contains("Space · Take delivery photo"));
        h["routes"][0]["state"] = json!("carrying");
        h["routes"][0]["rung"] = json!(true);
        assert!(delivery(&h).contains("F · Put camera away"));
    }
    #[test]
    fn inspecting_does_not_change_held_parcel() {
        let mut h = state();
        h["selected"] = json!(1);
        h["vanDistance"] = json!(30);
        h["routes"][0]["state"] = json!("carrying");
        assert!(delivery(&h).contains("Example 12"));
    }
    #[test]
    fn bite_precedes_parcel_controls() {
        let mut h = state();
        h["stunSeconds"] = json!(1.2);
        assert!(delivery(&h).contains("OUCH! · 1.2s"));
    }
}
