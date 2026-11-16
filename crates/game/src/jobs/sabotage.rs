use super::*;

pub(super) const MAX_AWARD: i32 = 400;
const GRACE: f32 = 45.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scenario {
    Feud,
    Overflow,
    Vacation,
    SleepingBaby,
    Fussy,
    Dog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pitch {
    Truth,
    DifferentName,
    Return,
    Urgent,
    Addressed,
}

pub(super) struct Attempt {
    pub scenario: Scenario,
    pub started: f32,
    pub neighbor: String,
    pub destination: Vec3,
    pub backup: bool,
    pub prepared: bool,
    pub answer_at: Option<f32>,
    pub recipient_at: Option<f32>,
}

impl Scenario {
    pub fn at(address: &str) -> Option<Self> {
        match service::hash(address) % 7 {
            3 => Some(Self::Feud),
            0 => Some(Self::Overflow),
            1 => Some(Self::Vacation),
            2 => Some(Self::SleepingBaby),
            4 => Some(Self::Fussy),
            5 => Some(Self::Dog),
            _ => None,
        }
    }

    fn base(self) -> i32 {
        match self {
            Self::Feud => 300,
            Self::Overflow => 240,
            Self::Vacation => 265,
            Self::SleepingBaby => 255,
            Self::Fussy => 230,
            Self::Dog => 210,
        }
    }

    fn required_pitch(self) -> Pitch {
        match self {
            Self::Feud => Pitch::DifferentName,
            Self::Overflow => Pitch::Return,
            Self::Vacation => Pitch::Urgent,
            Self::SleepingBaby | Self::Fussy => Pitch::Addressed,
            Self::Dog => Pitch::Truth,
        }
    }
}

impl Pitch {
    pub const OPTIONS: [Self; 5] = [
        Self::Truth,
        Self::DifferentName,
        Self::Return,
        Self::Urgent,
        Self::Addressed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Truth => "Tell the truth",
            Self::DifferentName => "Say it is for the building manager",
            Self::Return => "Say you are collecting a return",
            Self::Urgent => "Say it only takes a second",
            Self::Addressed => "Say the parcel is for them",
        }
    }
}

impl Attempt {
    pub fn evaluate(&self, now: f32, pitch: Pitch) -> Result<(), &'static str> {
        if self.backup {
            return if pitch == Pitch::Truth {
                Ok(())
            } else {
                Err("The backup neighbor refused. Try asking them directly.")
            };
        }
        if self.scenario == Scenario::Vacation && self.departed(now) {
            return Err("The neighbor has left for vacation.");
        }
        if pitch != self.scenario.required_pitch() {
            return Err(match self.scenario {
                Scenario::Feud => {
                    "The neighbor refused. They won't touch a parcel for this recipient."
                }
                Scenario::Overflow => {
                    "The neighbor refused. They're only opening for a returns pickup."
                }
                Scenario::Vacation => {
                    "The neighbor refused. They're leaving and only have a second."
                }
                Scenario::SleepingBaby | Scenario::Fussy => {
                    "The neighbor refused. They'll only take their own delivery."
                }
                Scenario::Dog => "The neighbor refused. Ask if they'll take it for the recipient.",
            });
        }
        if self.scenario == Scenario::Fussy && !self.prepared {
            return Err(
                "The neighbor refused. Set the box down at the marked spot by the real door first.",
            );
        }
        Ok(())
    }

    pub fn departed(&self, now: f32) -> bool {
        self.scenario == Scenario::Vacation && !self.backup && now - self.started >= 30.
    }

    pub fn award(&self, now: f32, distance: f32) -> i32 {
        let base = if self.backup
            || self.departed(now)
            || (self.scenario == Scenario::SleepingBaby
                && self.recipient_at.is_some_and(|at| now >= at))
        {
            80
        } else {
            self.scenario.base()
        };
        let distance_award = (distance * 2.0).clamp(0., 100.) as i32;
        let decay = ((now - self.started - GRACE).max(0.) * 12.0) as i32;
        (base + distance_award - decay).clamp(0, MAX_AWARD)
    }

    pub fn seconds(&self, now: f32) -> i32 {
        (GRACE - (now - self.started)).ceil().max(0.) as i32
    }

    pub fn message(&self, jobs: &Jobs, address: &str, label: &str) -> String {
        let neighbor = jobs.citizen(&self.neighbor).name;
        let recipient = jobs.citizen(address).name;
        let clue = if self.backup {
            "Ask directly. They'll take it at a lower rate."
        } else {
            match self.scenario {
                Scenario::Feud => "Say it's for the building manager.",
                Scenario::Overflow => {
                    "Their hall is full. They'll open if you say you're collecting a return."
                }
                Scenario::Vacation => {
                    "They're leaving in 30 seconds. Say it'll only take a second."
                }
                Scenario::SleepingBaby => {
                    "There's a sleeping baby at the real door. Tell the neighbor it's theirs before the recipient gets out."
                }
                Scenario::Fussy => {
                    "Set the box against the real door as requested, pick it up again, then tell the neighbor it's theirs."
                }
                Scenario::Dog => {
                    "Ask directly. Keep hold of the box. A dog may take it back to the real door."
                }
            }
        };
        if self.scenario == Scenario::Feud && !self.backup {
            format!("{neighbor} at {label} refuses parcels for {recipient}. {clue}")
        } else {
            format!("{neighbor} at {label}. {clue}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(scenario: Scenario) -> Attempt {
        Attempt {
            scenario,
            started: 10.,
            neighbor: "way/12".into(),
            destination: Vec3::X * 10.,
            backup: false,
            prepared: false,
            answer_at: None,
            recipient_at: Some(25.),
        }
    }

    #[test]
    fn each_neighbor_pitch_has_a_reason_and_an_outcome() {
        for (scene, pitch) in [
            (Scenario::Feud, Pitch::DifferentName),
            (Scenario::Overflow, Pitch::Return),
            (Scenario::Vacation, Pitch::Urgent),
            (Scenario::SleepingBaby, Pitch::Addressed),
            (Scenario::Dog, Pitch::Truth),
        ] {
            let a = attempt(scene);
            assert!(a.evaluate(12., pitch).is_ok(), "{scene:?}");
            assert!(a.evaluate(12., Pitch::Return).is_err() || scene == Scenario::Overflow);
        }
        let mut picky = attempt(Scenario::Fussy);
        assert!(picky.evaluate(12., Pitch::Addressed).is_err());
        picky.prepared = true;
        assert!(picky.evaluate(12., Pitch::Addressed).is_ok());
        assert!(
            attempt(Scenario::Vacation)
                .evaluate(41., Pitch::Urgent)
                .is_err()
        );
    }

    #[test]
    fn handover_award_depends_on_final_distance_and_decays_after_grace() {
        let a = attempt(Scenario::Feud);
        assert!(a.award(55., 50.) > a.award(55., 10.));
        assert_eq!(a.award(54., 50.), a.award(55., 50.));
        assert!(a.award(56., 50.) < a.award(55., 50.));
        assert_eq!(a.award(1000., 50.), 0);
        assert!(a.award(10., 1e6) <= MAX_AWARD);
        let sleeping = attempt(Scenario::SleepingBaby);
        assert!(sleeping.award(24., 20.) > sleeping.award(25., 20.));
    }
}
