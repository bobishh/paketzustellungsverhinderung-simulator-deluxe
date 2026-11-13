//! One shared workday and one shared vote, for solo and multiplayer alike.
use crate::{Ride, actors::Role, jobs::Jobs};
use bevy::prelude::*;
use paket_session::{Fact, Origin};
use std::collections::BTreeMap;

pub const DURATION_MS: u64 = 48 * 60 * 1000;
const VOTE_MS: u64 = 20_000;
#[derive(Clone)]
struct Ballot {
    sequence: u64,
    seen: web_time::Instant,
    courier: bool,
    score: i32,
    delivered: u32,
    finished: bool,
    choice: Option<u8>,
}
#[derive(Resource)]
pub(crate) struct Shift {
    anchor: web_time::Instant,
    elapsed: u64,
    day: u64,
    pub world: Option<Origin>,
    choices: Vec<Origin>,
    ballots: BTreeMap<String, Ballot>,
    choice: Option<u8>,
    sequence: u64,
    remote_remaining: u64,
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
            day: 1,
            world: None,
            choices: vec![],
            ballots: BTreeMap::new(),
            choice: None,
            sequence: 0,
            remote_remaining: VOTE_MS,
            ready: !guest,
            difficulty: 1,
            locked: false,
            broadcast: 2.,
        }
    }
    fn raw_elapsed(&self) -> u64 {
        if self.ready {
            self.elapsed
                .saturating_add(self.anchor.elapsed().as_millis() as u64)
        } else {
            0
        }
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.raw_elapsed().min(DURATION_MS)
    }
    pub fn day(&self) -> u64 {
        self.day
    }
    pub fn minute(&self) -> u32 {
        540 + (self.elapsed_ms() / 6000) as u32
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
    pub fn receive_snapshot(
        &mut self,
        day: u64,
        elapsed: u64,
        difficulty: u8,
        world: Option<Origin>,
        choices: Vec<Origin>,
        remaining: u64,
    ) {
        let valid = |o: &Origin| {
            o.lat.is_finite() && o.lon.is_finite() && o.lat.abs() <= 85. && o.lon.abs() <= 180.
        };
        if day < self.day
            || day > 36_500
            || elapsed > DURATION_MS
            || difficulty > 2
            || world.as_ref().is_some_and(|o| !valid(o))
            || !(choices.is_empty() || choices.len() == 3)
            || choices.iter().any(|o| !valid(o))
        {
            return;
        }
        if day > self.day {
            self.day = day;
            self.elapsed = 0;
            self.ready = false;
            self.choice = None;
            self.ballots.clear();
            self.locked = false;
        }
        self.receive(elapsed, difficulty);
        self.world = world;
        self.choices = choices;
        self.remote_remaining = remaining.min(VOTE_MS);
    }
    pub fn vote(&mut self, choice: u8) {
        if self.ended() && usize::from(choice) < self.choices.len() {
            self.choice = Some(choice);
            self.broadcast = 1.;
        }
    }
    pub fn receive_ballot(
        &mut self,
        device: String,
        day: u64,
        sequence: u64,
        courier: bool,
        score: i32,
        delivered: u32,
        finished: bool,
        choice: Option<u8>,
    ) {
        if day != self.day
            || !(-1_000_000..=1_000_000).contains(&score)
            || delivered > 100
            || choice.is_some_and(|c| c > 2)
            || self
                .ballots
                .get(&device)
                .is_some_and(|b| b.sequence >= sequence)
        {
            return;
        }
        if self.ballots.len() >= 32 && !self.ballots.contains_key(&device) {
            return;
        }
        self.ballots.insert(
            device,
            Ballot {
                sequence,
                seen: web_time::Instant::now(),
                courier,
                score,
                delivered,
                finished,
                choice,
            },
        );
    }
    fn remaining(&self, host: bool) -> u64 {
        if !self.ended() {
            return VOTE_MS;
        }
        if host {
            (DURATION_MS + VOTE_MS).saturating_sub(self.raw_elapsed())
        } else {
            self.remote_remaining
                .saturating_sub(self.anchor.elapsed().as_millis() as u64)
        }
    }
    fn winner(&self) -> usize {
        let mut counts = [0u32; 3];
        for b in self.ballots.values() {
            if let Some(i) = b.choice {
                counts[i as usize] += 1;
            }
        }
        let max = counts.iter().copied().max().unwrap_or(0);
        let winners = (0..3).filter(|&i| counts[i] == max).collect::<Vec<_>>();
        if winners.len() == 1 { winners[0] } else { 0 }
    }
    pub fn hud(&self, ride: &Ride) -> serde_json::Value {
        let own = ride.session.as_ref().map_or("solo", |s| s.device.as_str());
        let host = ride.session.as_ref().map_or(!ride.guest, |s| s.is_host);
        serde_json::json!({"day":self.day,"ended":self.ended(),"remaining":self.remaining(host).div_ceil(1000),"choice":self.choice,"options":self.choices.iter().enumerate().map(|(i,o)|if i==0{"Same neighbourhood".to_string()}else{crate::map::location_name((*o).into())}).collect::<Vec<_>>(),"players":self.ballots.iter().map(|(id,b)|serde_json::json!({"name":if id==own{"You".to_string()}else{format!("{} {}",if b.courier{"Courier"}else{"Dog"},id.chars().take(6).collect::<String>())},"score":b.score,"delivered":b.delivered,"vote":b.choice})).collect::<Vec<_>>()})
    }
    pub fn label(&self) -> String {
        if !self.ready {
            return "Synchronizing the shift with the host…".into();
        }
        let m = self.minute();
        format!(
            "Day {} · {:02}:{:02} / 09:00–17:00 · {}{}",
            self.day,
            m / 60,
            m % 60,
            ["Trainee", "Courier", "Express"][self.difficulty as usize],
            if self.ended() {
                " · Vote: 1 same area / 2 or 3 random area"
            } else if !self.locked {
                " · F2 difficulty"
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
    jobs: Res<Jobs>,
    mut shift: ResMut<Shift>,
) {
    let host = ride.session.as_ref().map_or(!ride.guest, |s| s.is_host);
    if !shift.ready {
        return;
    }
    if host && !shift.locked && keys.just_pressed(KeyCode::F2) {
        shift.difficulty = (shift.difficulty + 1) % 3;
    }
    shift.ballots.retain(|_, b| b.seen.elapsed().as_secs() < 10);
    let own = ride
        .session
        .as_ref()
        .map_or_else(|| "solo".into(), |s| s.device.clone());
    shift.sequence += 1;
    let sequence = shift.sequence;
    let day = shift.day;
    let choice = shift.choice;
    let progress = jobs.progress();
    let score = progress["score"].as_i64().unwrap_or(0) as i32;
    let delivered = progress["delivered"].as_u64().unwrap_or(0) as u32;
    shift.receive_ballot(
        own.clone(),
        day,
        sequence,
        ride.role == Role::Courier,
        score,
        delivered,
        jobs.complete(),
        choice,
    );
    let all_done = shift.ballots.values().any(|b| b.courier)
        && shift
            .ballots
            .values()
            .filter(|b| b.courier)
            .all(|b| b.finished);
    if host && jobs.day() == shift.day && all_done && !shift.ended() {
        shift.elapsed = DURATION_MS;
        shift.anchor = web_time::Instant::now();
    }
    if host && shift.ended() && shift.choices.is_empty() {
        let origin = ride.origin;
        let mut pool = crate::map::CITIES
            .iter()
            .map(|(_, p)| *p)
            .filter(|p| (p.lat - origin.lat).abs() + (p.lon - origin.lon).abs() > 0.02)
            .collect::<Vec<_>>();
        let salt = own.bytes().fold(shift.day, |h, b| {
            h.wrapping_mul(31).wrapping_add(u64::from(b))
        });
        let first = pool.remove(salt as usize % pool.len());
        let second = pool.remove((salt / 7) as usize % pool.len());
        shift.choices = vec![origin.into(), first.into(), second.into()];
        shift.broadcast = 1.;
    }
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            shift.vote(i as u8);
        }
    }
    let voted = shift.ballots.values().all(|b| b.choice.is_some());
    if host
        && shift.ended()
        && shift.choices.len() == 3
        && (shift.remaining(true) == 0 || (voted && shift.raw_elapsed() >= DURATION_MS + 1000))
    {
        let winner = shift.winner();
        shift.world = Some(shift.choices[winner]);
        shift.day += 1;
        shift.elapsed = 0;
        shift.anchor = web_time::Instant::now();
        shift.choices.clear();
        shift.ballots.clear();
        shift.choice = None;
        shift.locked = false;
        shift.broadcast = 1.;
    }
    shift.broadcast += time.delta_secs();
    if shift.broadcast >= 1. {
        shift.broadcast = 0.;
        if let Some(s) = &ride.session {
            if host {
                let _ = s.publish(Fact::Shift {
                    origin: s.origin,
                    device: s.device.clone(),
                    day: shift.day,
                    elapsed_ms: shift.elapsed_ms(),
                    difficulty: shift.difficulty,
                    world: Some(shift.world.unwrap_or(ride.origin.into())),
                    choices: shift.choices.clone(),
                    remaining_ms: shift.remaining(true),
                });
            }
            if jobs.day() == shift.day {
                let _ = s.publish(Fact::Ballot {
                    origin: s.origin,
                    device: s.device.clone(),
                    day: shift.day,
                    sequence,
                    courier: ride.role == Role::Courier,
                    score,
                    delivered,
                    finished: jobs.complete(),
                    choice: shift.choice,
                });
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_clock_waits_for_a_vote_and_rejects_old_days() {
        let mut s = Shift::new(true);
        assert!(!s.ready);
        assert_eq!(s.remaining(true), VOTE_MS);
        s.receive(180000, 2);
        assert_eq!(s.minute(), 570);
        assert_eq!(s.remaining(true), VOTE_MS);
        s.receive(DURATION_MS, 1);
        assert!(s.ended());
        s.receive_snapshot(2, 0, 1, None, vec![], 0);
        assert_eq!(s.day(), 2);
        assert_eq!(s.minute(), 540);
        assert!(!s.ended());
        s.receive_snapshot(1, DURATION_MS, 0, None, vec![], 0);
        assert_eq!(s.day(), 2);
        assert_eq!(s.difficulty, 1);
    }
    #[test]
    fn vote_majority_wins_and_ties_stay_in_place() {
        let mut s = Shift::default();
        s.receive_ballot("a".into(), 1, 1, true, 100, 12, true, Some(1));
        s.receive_ballot("b".into(), 1, 1, true, 0, 12, true, Some(0));
        assert_eq!(s.winner(), 0);
        s.receive_ballot("c".into(), 1, 1, false, 40, 0, false, Some(1));
        assert_eq!(s.winner(), 1);
        s.receive_ballot("c".into(), 1, 0, false, 40, 0, false, Some(2));
        assert_eq!(s.winner(), 1);
    }
}
