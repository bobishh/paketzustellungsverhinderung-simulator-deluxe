use paket_geo::LatLon;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub lat: f64,
    pub lon: f64,
}

impl Origin {
    pub(crate) fn matches(self, other: Self) -> bool {
        // A JSON round trip can change a computed coordinate by a few floating-point bits.
        (self.lat - other.lat).abs() <= 1e-9 && (self.lon - other.lon).abs() <= 1e-9
    }
}

impl From<LatLon> for Origin {
    fn from(value: LatLon) -> Self {
        Self {
            lat: value.lat,
            lon: value.lon,
        }
    }
}

impl From<Origin> for LatLon {
    fn from(value: Origin) -> Self {
        Self {
            lat: value.lat,
            lon: value.lon,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Avatar {
    Courier,
    Dog,
    Van,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VanPose {
    pub east: f32,
    pub north: f32,
    #[serde(default = "legacy_ground_height")]
    pub up: f32,
    pub heading: f32,
}

fn legacy_ground_height() -> f32 {
    -0.04
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorPose {
    pub avatar: Avatar,
    pub east: f32,
    pub north: f32,
    pub up: f32,
    pub heading: f32,
    pub velocity_east: f32,
    pub velocity_north: f32,
    pub parked_van: Option<VanPose>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum StreetAction {
    Door {
        building_id: String,
    },
    Bite {
        target: String,
    },
    MarkVan {
        owner: String,
        east: f32,
        north: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub device: String,
    pub photo: bool,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteStop {
    pub building_id: String,
    pub label: String,
    pub position: [f32; 3],
    pub completion: Option<DeliveryReceipt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fact {
    Shift {
        origin: Origin,
        device: String,
        #[serde(default = "first_day")]
        day: u64,
        elapsed_ms: u64,
        difficulty: u8,
        #[serde(default)]
        world: Option<Origin>,
        #[serde(default)]
        choices: Vec<Origin>,
        #[serde(default)]
        remaining_ms: u64,
    },
    Ballot {
        origin: Origin,
        device: String,
        day: u64,
        sequence: u64,
        courier: bool,
        score: i32,
        delivered: u32,
        finished: bool,
        choice: Option<u8>,
    },
    Actor {
        origin: Origin,
        device: String,
        sequence: u64,
        pose: ActorPose,
    },
    StreetAction {
        origin: Origin,
        #[serde(default = "first_day")]
        day: u64,
        device: String,
        sequence: u64,
        event: StreetAction,
    },
    Route {
        origin: Origin,
        device: String,
        day: u64,
        revision: u64,
        stops: Vec<RouteStop>,
    },
    Delivered {
        origin: Origin,
        device: String,
        day: u64,
        building_id: String,
        photo: bool,
        score: i32,
    },
    Delivery {
        origin: Origin,
        building_id: String,
    },
    Pose {
        origin: Origin,
        device: String,
        east: f32,
        north: f32,
        heading: f32,
        #[serde(default = "legacy_bike_height")]
        up: f32,
        #[serde(default)]
        velocity_east: f32,
        #[serde(default)]
        velocity_north: f32,
        #[serde(default)]
        sequence: u64,
    },
}

fn legacy_bike_height() -> f32 {
    0.7
}

fn first_day() -> u64 {
    1
}

impl Fact {
    pub fn origin(&self) -> Origin {
        match self {
            Self::Route { origin, .. }
            | Self::Delivered { origin, .. }
            | Self::Delivery { origin, .. }
            | Self::Shift { origin, .. }
            | Self::Ballot { origin, .. }
            | Self::Pose { origin, .. }
            | Self::Actor { origin, .. }
            | Self::StreetAction { origin, .. } => *origin,
        }
    }

    pub fn device(&self) -> Option<&str> {
        match self {
            Self::Route { device, .. }
            | Self::Delivered { device, .. }
            | Self::Pose { device, .. }
            | Self::Shift { device, .. }
            | Self::Ballot { device, .. }
            | Self::Actor { device, .. }
            | Self::StreetAction { device, .. } => Some(device),
            Self::Delivery { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parked_van_elevation_round_trips_and_old_peers_stay_on_the_ground() {
        let pose: VanPose = serde_json::from_str(r#"{"east":1,"north":2,"heading":0}"#).unwrap();
        assert_eq!(pose.up, -0.04);
        let elevated = VanPose { up: 2.345, ..pose };
        let decoded: VanPose =
            serde_json::from_str(&serde_json::to_string(&elevated).unwrap()).unwrap();
        assert_eq!(decoded, elevated);
    }
}
