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
    pub heading: f32,
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
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fact {
    Actor {
        origin: Origin,
        device: String,
        sequence: u64,
        pose: ActorPose,
    },
    StreetAction {
        origin: Origin,
        device: String,
        sequence: u64,
        event: StreetAction,
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

impl Fact {
    pub fn origin(&self) -> Origin {
        match self {
            Self::Delivery { origin, .. }
            | Self::Pose { origin, .. }
            | Self::Actor { origin, .. }
            | Self::StreetAction { origin, .. } => *origin,
        }
    }

    pub fn device(&self) -> Option<&str> {
        match self {
            Self::Pose { device, .. }
            | Self::Actor { device, .. }
            | Self::StreetAction { device, .. } => Some(device),
            Self::Delivery { .. } => None,
        }
    }
}
