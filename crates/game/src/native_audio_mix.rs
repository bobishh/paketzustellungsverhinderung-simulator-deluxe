//! Recorded audio follows committed motion and game state, not keyboard intent.
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct Sound {
    pub name: String,
    pub gain: f32,
    pub pan: f32,
    pub rate: f32,
    pub variant: Option<usize>,
}
impl Sound {
    fn named(name: &str) -> Self {
        Self {
            name: name.into(),
            gain: 1.,
            pan: 0.,
            rate: 1.,
            variant: None,
        }
    }
}
#[derive(Default)]
pub(crate) struct Mix {
    pub active: bool,
    pub effects: Vec<Sound>,
    pub loops: Vec<Sound>,
}
#[derive(Default)]
pub(crate) struct Mixer {
    pub muted: bool,
    ride: bool,
    street: Value,
    round: Value,
    position: Option<[f64; 3]>,
    distance: f64,
    effects: Vec<Sound>,
    callers: HashMap<String, f64>,
    character: String,
    next_voice: f64,
    routes: HashMap<u64, String>,
    photo: bool,
    blocked: bool,
}
impl Mixer {
    pub fn event(&mut self, name: &str, value: &Value, now: f64) {
        match name {
            "paket-mode" => {
                let muted = self.muted;
                *self = Self::default();
                self.muted = muted;
                self.ride = value.as_str() == Some("ride");
            }
            "street-audio" => {
                self.street = value.clone();
                let position = value["position"]
                    .as_array()
                    .and_then(|p| {
                        Some([
                            p.first()?.as_f64()?,
                            p.get(1)?.as_f64()?,
                            p.get(2)?.as_f64()?,
                        ])
                    })
                    .filter(|p| p.iter().all(|v| v.is_finite()));
                let previous = self.position.replace(position.unwrap_or([f64::NAN; 3]));
                let moving_on_foot = value["active"] == true
                    && value["form"] != "van"
                    && number(&value["verticalSpeed"]).abs() <= 1.5;
                if let (Some(previous), Some(position)) = (previous, position) {
                    let delta = (position[0] - previous[0]).hypot(position[2] - previous[2]);
                    if moving_on_foot && delta.is_finite() && delta <= 3. {
                        self.distance += delta;
                        let stride = if value["form"] == "dog" { 0.48 } else { 0.8 };
                        if self.distance >= stride {
                            self.distance %= stride;
                            self.push(Sound::named("step-pavement"));
                        }
                    } else {
                        self.distance = 0.;
                    }
                } else {
                    self.distance = 0.;
                }
            }
            "game-sound" => {
                if let Some(name) = value.as_str() {
                    self.push(Sound::named(name));
                }
            }
            "paket-bite" => self.push(Sound::named("bite")),
            "paket-hud" if self.round.is_null() => {
                let mut routes = HashMap::new();
                for route in value["routes"].as_array().into_iter().flatten() {
                    let Some(index) = route["index"].as_u64() else {
                        continue;
                    };
                    let Some(state) = route["state"].as_str() else {
                        continue;
                    };
                    if self.routes.get(&index).is_some_and(|old| old != state) {
                        let name = match state {
                            "carrying" => Some("parcel"),
                            "in-flight" => Some("cloth"),
                            "set-aside" | "photo-due" => Some("soft-hit"),
                            "filed" | "handed" => Some("success"),
                            _ => None,
                        };
                        if let Some(name) = name {
                            self.push(Sound::named(name));
                        }
                    }
                    routes.insert(index, state.to_string());
                }
                self.routes = routes;
                let ready = value["photoReady"] == true;
                if ready && !self.photo {
                    self.push(Sound::named("camera"));
                }
                self.photo = ready;
            }
            "rich-round" => {
                self.round = value.clone();
                if !self.audible() || !value["tutorial"].is_null() || self.blocked {
                    self.reset_callers();
                    return;
                }
                let character = value["character"].as_str().unwrap_or("");
                if self.character != character {
                    self.reset_callers();
                    self.character = character.to_string();
                }
                let people = value["soundCrowd"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                self.callers
                    .retain(|id, _| people.iter().any(|p| p["id"].as_str() == Some(id)));
                if now < self.next_voice {
                    return;
                }
                for person in people {
                    let Some(id) = person["id"].as_str() else {
                        continue;
                    };
                    let hash = id.bytes().fold(7u32, |hash, byte| {
                        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
                    });
                    let Some(due) = self.callers.get_mut(id) else {
                        self.callers
                            .insert(id.into(), now + 0.8 + f64::from(hash % 19) / 10.);
                        continue;
                    };
                    let Some(distance) = person["distance"]
                        .as_f64()
                        .filter(|d| d.is_finite() && *d <= 30.)
                    else {
                        continue;
                    };
                    if now < *due {
                        continue;
                    }
                    *due = now + 3.5 + f64::from(hash % 29) / 10.;
                    self.next_voice = now + 0.6;
                    let mut sound = Sound::named(&format!(
                        "call-{}",
                        character
                            .to_lowercase()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join("-")
                    ));
                    sound.gain = (0.9 / (1. + distance.max(0.) / 9.)).min(0.85) as f32;
                    sound.pan = number(&person["pan"]).clamp(-1., 1.) as f32;
                    sound.variant = Some((hash % 3) as usize);
                    self.push(sound);
                    break;
                }
            }
            _ => {}
        }
    }
    fn audible(&self) -> bool {
        self.ride
            && !self.muted
            && self.street["active"] == true
            && self.round["finished"] != true
            && number(&self.round["opening"]) <= 0.
    }
    fn push(&mut self, sound: Sound) {
        if self.effects.len() < 16 {
            self.effects.push(sound);
        }
    }
    fn reset_callers(&mut self) {
        self.callers.clear();
        self.character.clear();
        self.next_voice = 0.;
    }
    pub fn frame(&mut self, allowed: bool) -> Mix {
        self.blocked = !allowed;
        if !allowed || !self.audible() {
            self.effects.clear();
            self.position = None;
            self.distance = 0.;
            self.reset_callers();
            return Mix::default();
        }
        let mut street = Sound::named("street");
        street.gain = 0.55;
        let mut loops = vec![street];
        if self.street["form"] == "van" {
            loops.extend(engine_layers(
                number(&self.street["speed"]),
                number(&self.street["throttle"]),
            ));
        }
        let car = &self.street["traffic"];
        if let Some(distance) = car["distance"]
            .as_f64()
            .filter(|d| d.is_finite() && *d < 42.)
        {
            let mut engine = Sound::named("engine");
            engine.gain = (0.45 * (1. - distance.max(0.) / 42.).powi(2)) as f32;
            engine.pan = number(&car["pan"]).clamp(-1., 1.) as f32;
            engine.rate = (0.85 + (number(&car["speed"]) / 45.).clamp(0., 1.) * 0.35) as f32;
            loops.push(engine);
        }
        Mix {
            active: true,
            effects: std::mem::take(&mut self.effects),
            loops,
        }
    }
}
fn number(value: &Value) -> f64 {
    value.as_f64().filter(|v| v.is_finite()).unwrap_or(0.)
}
pub(crate) fn engine_layers(speed: f64, throttle: f64) -> Vec<Sound> {
    let speed = if speed.is_finite() { speed.abs() } else { 0. };
    let load = if throttle.is_finite() {
        throttle.abs().clamp(0., 1.)
    } else {
        0.
    };
    let gears = [0., 18., 40., 70., 110., 160.];
    let gear = gears
        .iter()
        .position(|limit| *limit > speed)
        .unwrap_or(gears.len() - 1)
        .max(1);
    let fraction = ((speed - gears[gear - 1]) / (gears[gear] - gears[gear - 1])).clamp(0., 1.);
    let rpm = (if speed < 2. {
        load * 0.38
    } else {
        0.22 + fraction * 0.65 + load * 0.08
    })
    .clamp(0., 1.);
    let idle = 1. - (rpm / 0.27).clamp(0., 1.);
    let blend = ((rpm - 0.25) / 0.65).clamp(0., 1.);
    let power = 0.4 + load * 0.6;
    [
        ("engine-idle", 0.6 * idle.sqrt(), 0.9 + rpm * 0.3),
        (
            "engine-low",
            0.8 * (1. - idle) * (1. - blend).sqrt() * power,
            0.78 + rpm * 0.55,
        ),
        (
            "engine-high",
            0.8 * (1. - idle) * blend.sqrt() * power,
            0.74 + rpm * 0.5,
        ),
    ]
    .into_iter()
    .map(|(name, gain, rate)| Sound {
        name: name.into(),
        gain: gain as f32,
        rate: rate as f32,
        pan: 0.,
        variant: None,
    })
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn street(x: f64, speed: f64) -> Value {
        json!({"active":true,"position":[x,0.,0.],"speed":speed,"verticalSpeed":0.,"form":"courier","throttle":0.,"traffic":null})
    }
    fn live() -> Mixer {
        let mut mix = Mixer::default();
        mix.event("paket-mode", &json!("ride"), 0.);
        mix.event("street-audio", &street(0., 0.), 0.);
        mix.frame(true);
        mix
    }
    #[test]
    fn footsteps_require_real_distance_and_skip_teleports_and_airborne_motion() {
        let mut mix = live();
        mix.event("street-audio", &street(0., 100.), 0.1);
        assert!(mix.frame(true).effects.is_empty());
        mix.event("street-audio", &street(0.5, 5.), 0.2);
        assert!(mix.frame(true).effects.is_empty());
        mix.event("street-audio", &street(0.9, 5.), 0.3);
        assert_eq!(mix.frame(true).effects[0].name, "step-pavement");
        mix.event("street-audio", &street(100., 5.), 0.4);
        assert!(mix.frame(true).effects.is_empty());
        let mut air = street(101., 5.);
        air["verticalSpeed"] = json!(3.);
        mix.event("street-audio", &air, 0.5);
        assert!(mix.frame(true).effects.is_empty());
    }
    #[test]
    fn mute_loading_background_and_finished_rounds_clear_pending_effects_and_loops() {
        let mut mix = live();
        assert!(mix.frame(true).active);
        mix.muted = true;
        mix.event("game-sound", &json!("throw-cash"), 1.);
        let quiet = mix.frame(true);
        assert!(quiet.effects.is_empty() && quiet.loops.is_empty());
        mix.muted = false;
        mix.event("game-sound", &json!("throw-cash"), 1.);
        let quiet = mix.frame(false);
        assert!(quiet.effects.is_empty() && quiet.loops.is_empty());
        mix.muted = false;
        assert!(
            mix.frame(true).effects.is_empty(),
            "resume must not replay a paused throw"
        );
        mix.event("rich-round", &json!({"finished":true}), 2.);
        assert!(!mix.frame(true).active);
    }
    #[test]
    fn recorded_engine_mix_changes_with_actual_speed_and_gas() {
        let idle = engine_layers(0., 0.);
        let gas = engine_layers(0., 1.);
        let moving = engine_layers(35., 1.);
        assert!(idle[0].gain > gas[0].gain);
        assert!(moving[2].gain > idle[2].gain);
        assert!(gas[0].rate > idle[0].rate);
    }
    #[test]
    fn named_crowd_calls_use_selected_profile_distance_pan_and_cooldowns() {
        let mut mix = live();
        let crowd = json!({"character":"Steve Ballmer","opening":0,"finished":false,"tutorial":null,"soundCrowd":[{"id":"visitor-1","distance":6.,"pan":-0.7}]});
        mix.event("rich-round", &crowd, 0.);
        assert!(mix.frame(true).effects.is_empty());
        mix.event("rich-round", &crowd, 3.);
        let result = mix.frame(true);
        assert_eq!(result.effects.len(), 1);
        assert_eq!(result.effects[0].name, "call-steve-ballmer");
        assert!(result.effects[0].pan < 0. && result.effects[0].gain > 0.);
        mix.event("rich-round", &crowd, 3.2);
        assert!(mix.frame(true).effects.is_empty());
        let mut tutorial = crowd.clone();
        tutorial["tutorial"] = json!({"index":1});
        mix.event("rich-round", &tutorial, 20.);
        assert!(mix.frame(true).effects.is_empty());
    }
    #[test]
    fn all_nine_profiles_select_their_three_approved_recordings() {
        for (name, id) in [
            ("Elon Musk", "elon-musk"),
            ("Jeff Bezos", "jeff-bezos"),
            ("Larry Page", "larry-page"),
            ("Sergey Brin", "sergey-brin"),
            ("Mark Zuckerberg", "mark-zuckerberg"),
            ("Michael Dell", "michael-dell"),
            ("Warren Buffett", "warren-buffett"),
            ("Steve Ballmer", "steve-ballmer"),
            ("Larry Ellison", "larry-ellison"),
        ] {
            let mut variants = std::collections::HashSet::new();
            for visitor in 0..3 {
                let mut mix = live();
                let crowd = json!({"character":name,"opening":0,"finished":false,"tutorial":null,
                    "soundCrowd":[{"id":format!("visitor-{visitor}"),"distance":4.,"pan":0.5}]});
                mix.event("rich-round", &crowd, 0.);
                mix.frame(true);
                mix.event("rich-round", &crowd, 3.);
                let frame = mix.frame(true);
                assert_eq!(frame.effects.len(), 1, "{name}");
                assert_eq!(frame.effects[0].name, format!("call-{id}"));
                variants.insert(frame.effects[0].variant.unwrap());
            }
            assert_eq!(
                variants.len(),
                3,
                "{name} must select all three existing calls"
            );
        }
    }
    #[test]
    fn package_cues_follow_committed_route_transitions_and_photo_readiness() {
        let mut mix = live();
        mix.event(
            "paket-hud",
            &json!({"routes":[{"index":1,"state":"carrying"}],"photoReady":false}),
            0.,
        );
        assert!(mix.frame(true).effects.is_empty());
        mix.event(
            "paket-hud",
            &json!({"routes":[{"index":1,"state":"handed"}],"photoReady":true}),
            1.,
        );
        let result = mix.frame(true);
        assert!(result.effects.iter().any(|cue| cue.name == "success"));
        assert!(result.effects.iter().any(|cue| cue.name == "camera"));
    }
}
