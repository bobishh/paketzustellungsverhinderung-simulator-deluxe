//! One host clock for the whole ride. Eight in-game hours take 48 real minutes.
use crate::Ride;
use bevy::prelude::*;
use paket_session::Fact;

pub const DURATION_MS: u64 = 48 * 60 * 1000;
#[derive(Resource)]
pub(crate) struct Shift {
    anchor: web_time::Instant,
    elapsed: u64,
    pub ready: bool,
    pub difficulty: u8,
    pub locked: bool,
    broadcast: f32,
}
impl Default for Shift {
    fn default() -> Self {
        Self::new(false)
    }
}
impl Shift {
    pub fn new(guest: bool) -> Self {
        Self {
            anchor: web_time::Instant::now(),
            elapsed: 0,
            ready: !guest,
            difficulty: 1,
            locked: false,
            broadcast: 2.0,
        }
    }
    pub fn elapsed_ms(&self) -> u64 {
        if self.ready {
            (self.elapsed + self.anchor.elapsed().as_millis() as u64).min(DURATION_MS)
        } else {
            0
        }
    }
    pub fn minute(&self) -> u32 {
        9 * 60 + (self.elapsed_ms() / 6000) as u32
    }
    pub fn ended(&self) -> bool {
        self.elapsed_ms() >= DURATION_MS
    }
    pub fn receive(&mut self, elapsed: u64, difficulty: u8) {
        if elapsed > DURATION_MS || difficulty > 2 {
            return;
        }
        self.elapsed = elapsed.max(self.elapsed_ms());
        self.anchor = web_time::Instant::now();
        self.ready = true;
        self.difficulty = difficulty;
    }
    pub fn label(&self) -> String {
        if !self.ready {
            return "Synchronizing the shift with the host…".into();
        }
        let m = self.minute();
        format!(
            "{:02}:{:02}  /  09:00–17:00  ·  {}{}",
            m / 60,
            m % 60,
            ["Trainee", "Courier", "Express"][self.difficulty as usize],
            if self.ended() {
                "  ·  OFF DUTY"
            } else if !self.locked {
                "  ·  F2 difficulty"
            } else {
                ""
            }
        )
    }
}
pub(crate) fn tick(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    ride: Res<Ride>,
    mut shift: ResMut<Shift>,
) {
    let host = ride.session.as_ref().map_or(!ride.guest, |s| s.is_host);
    if host && !shift.locked && keys.just_pressed(KeyCode::F2) {
        shift.difficulty = (shift.difficulty + 1) % 3;
    }
    shift.broadcast += time.delta_secs();
    if host && shift.broadcast >= 1.0 {
        shift.broadcast = 0.0;
        if let Some(session) = &ride.session {
            let _ = session.publish(Fact::Shift {
                origin: ride.origin.into(),
                device: session.device.clone(),
                elapsed_ms: shift.elapsed_ms(),
                difficulty: shift.difficulty,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_shift_has_one_bounded_workday() {
        let mut s = Shift::new(true);
        assert!(!s.ready);
        assert_eq!(s.minute(), 540);
        s.receive(180000, 2);
        assert_eq!(s.minute(), 570);
        assert_eq!(s.difficulty, 2);
        s.receive(1000, 0);
        assert!(s.elapsed_ms() >= 180000);
        s.receive(DURATION_MS, 1);
        assert_eq!(s.minute(), 1020);
        assert!(s.ended());
    }
}
