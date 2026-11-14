//! The host owns route order and immutable delivery receipts; parcels project that state locally.
use super::*;
use paket_session::{DeliveryReceipt, RouteStop};

pub(super) struct Sharing {
    pub device: String,
    pub host: bool,
    pub revision: u64,
    pub received: Option<(u64, u64)>,
    pub timer: f32,
}
impl Jobs {
    pub fn configure_shared(&mut self, device: &str, host: bool) {
        self.sharing.get_or_insert_with(|| Sharing {
            device: device.into(),
            host,
            revision: 1,
            received: None,
            timer: 0.,
        });
    }
    pub(super) fn credited(&self, parcel: &Parcel) -> bool {
        self.sharing.as_ref().is_none_or(|s| {
            parcel
                .completion
                .as_ref()
                .is_some_and(|r| r.device == s.device)
        })
    }
    pub(super) fn route_changed(&mut self) {
        if let Some(s) = &mut self.sharing {
            s.revision += 1;
        }
    }
    pub(super) fn reset_shared_day(&mut self) {
        for p in &mut self.parcels {
            p.completion = None;
        }
        if let Some(s) = &mut self.sharing {
            s.revision += 1;
            s.received = None;
            s.timer = 0.;
        }
    }
    fn advance_shared_selection(&mut self) {
        let done = |p: &Parcel| matches!(p.state, ParcelState::Filed(_) | ParcelState::Handed);
        if self.parcels.get(self.navigation).is_some_and(done) {
            if let Some(next) = self.parcels.iter().position(|p| !done(p)) {
                self.navigation = next;
            }
        }
        if self.parcels.get(self.selected).is_some_and(done) {
            self.selected = self.navigation;
            self.proof_pending = None;
            self.photo_ready = false;
            self.pursued = false;
        }
    }
    pub fn receive_route(
        &mut self,
        day: u64,
        revision: u64,
        stops: Vec<RouteStop>,
        commands: &mut Commands,
    ) -> bool {
        let Some(sharing) = &self.sharing else {
            return false;
        };
        if sharing.host
            || day != self.day()
            || sharing
                .received
                .is_some_and(|(d, r)| d == day && r >= revision)
            || stops.len() > DELIVERIES_PER_SHIFT
            || !valid_stops(&stops)
        {
            return false;
        }
        // A shift's route only grows. A newer snapshot cannot resurrect a completed stop.
        if sharing.received.is_some_and(|(d, _)| d == day)
            && (stops.len() < self.parcels.len()
                || self
                    .parcels
                    .iter()
                    .zip(&stops)
                    .any(|(p, s)| p.address != s.building_id))
        {
            return false;
        }
        let mut previous: HashMap<_, _> = std::mem::take(&mut self.parcels)
            .into_iter()
            .map(|p| (p.address.clone(), p))
            .collect();
        for stop in stops {
            let mut parcel = previous.remove(&stop.building_id).unwrap_or(Parcel {
                address: stop.building_id,
                label: stop.label.clone(),
                point: Vec3::from_array(stop.position),
                state: ParcelState::InVan,
                completion: None,
                dropped_at: None,
                prop: None,
            });
            parcel.label = stop.label;
            parcel.point = Vec3::from_array(stop.position);
            if let Some(receipt) = parcel.completion.clone().or(stop.completion) {
                complete(&mut parcel, receipt, commands);
            }
            self.parcels.push(parcel);
        }
        for mut removed in previous.into_values() {
            if let Some(prop) = removed.prop.take() {
                commands.entity(prop).try_despawn();
            }
        }
        self.sharing.as_mut().unwrap().received = Some((day, revision));
        self.selected = self.selected.min(self.parcels.len().saturating_sub(1));
        self.navigation = self.navigation.min(self.parcels.len().saturating_sub(1));
        self.advance_shared_selection();
        true
    }
    pub fn confirm_delivery(
        &mut self,
        day: u64,
        building: &str,
        receipt: DeliveryReceipt,
        commands: &mut Commands,
    ) -> bool {
        if day != self.day()
            || !self.sharing.as_ref().is_some_and(|s| s.host)
            || !valid_receipt(&receipt)
        {
            return false;
        }
        let Some(parcel) = self.parcels.iter_mut().find(|p| p.address == building) else {
            return false;
        };
        if parcel.completion.is_some() {
            return false;
        }
        complete(parcel, receipt, commands);
        self.route_changed();
        self.advance_shared_selection();
        true
    }
    pub fn publish_shared(
        &mut self,
        session: &paket_session::SharedStreet,
        delta: f32,
        commands: &mut Commands,
    ) {
        let pending: Vec<_> = self
            .parcels
            .iter()
            .filter_map(|p| {
                if p.completion.is_some() {
                    return None;
                }
                let (photo, score) = match p.state {
                    ParcelState::Filed(score) => (true, score),
                    ParcelState::Handed => (false, 0),
                    _ => return None,
                };
                Some((
                    p.address.clone(),
                    DeliveryReceipt {
                        device: session.device.clone(),
                        photo,
                        score,
                    },
                ))
            })
            .collect();
        if session.is_host {
            for (address, receipt) in &pending {
                self.confirm_delivery(self.day(), address, receipt.clone(), commands);
            }
        }
        let Some(sharing) = &mut self.sharing else {
            return;
        };
        sharing.timer -= delta;
        if sharing.timer > 0. {
            return;
        }
        sharing.timer = 0.5;
        if session.is_host {
            let revision = sharing.revision;
            let stops = self
                .parcels
                .iter()
                .map(|p| RouteStop {
                    building_id: p.address.clone(),
                    label: p.label.clone(),
                    position: p.point.to_array(),
                    completion: p.completion.clone(),
                })
                .collect();
            let _ = session.publish(Fact::Route {
                origin: session.origin,
                device: session.device.clone(),
                day: self.day(),
                revision,
                stops,
            });
        } else {
            for (building_id, receipt) in pending {
                let _ = session.publish(Fact::Delivered {
                    origin: session.origin,
                    device: session.device.clone(),
                    day: self.day(),
                    building_id,
                    photo: receipt.photo,
                    score: receipt.score,
                });
            }
        }
    }
}

fn valid_receipt(r: &DeliveryReceipt) -> bool {
    !r.device.is_empty()
        && r.device.len() <= 256
        && matches!(r.score, 0 | 25 | 100)
        && (r.photo || r.score == 0)
}
fn valid_stops(stops: &[RouteStop]) -> bool {
    let mut ids = HashSet::new();
    stops.iter().all(|s| {
        !s.building_id.is_empty()
            && s.building_id.len() <= 256
            && ids.insert(&s.building_id)
            && !s.label.is_empty()
            && s.label.len() <= 1024
            && s.position.iter().all(|n| n.is_finite() && n.abs() <= 1e7)
            && s.completion.as_ref().is_none_or(valid_receipt)
    })
}
fn complete(parcel: &mut Parcel, receipt: DeliveryReceipt, commands: &mut Commands) {
    if let Some(prop) = parcel.prop.take() {
        commands.entity(prop).try_despawn();
    }
    parcel.state = if receipt.photo {
        ParcelState::Filed(receipt.score)
    } else {
        ParcelState::Handed
    };
    parcel.completion = Some(receipt);
    parcel.dropped_at = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt(device: &str) -> DeliveryReceipt {
        DeliveryReceipt {
            device: device.into(),
            photo: true,
            score: 100,
        }
    }
    fn stops() -> Vec<RouteStop> {
        vec![
            RouteStop {
                building_id: "way/1".into(),
                label: "Shared first address".into(),
                position: [10., 0., 20.],
                completion: Some(receipt("host")),
            },
            RouteStop {
                building_id: "way/2".into(),
                label: "Shared second address".into(),
                position: [30., 0., 40.],
                completion: None,
            },
        ]
    }
    #[test]
    fn late_join_uses_host_addresses_and_completed_stops_without_earning_another_players_score() {
        let mut world = World::new();
        let mut jobs = Jobs::default();
        jobs.day = 3;
        jobs.configure_shared("guest", false);
        assert!(jobs.receive_route(3, 7, stops(), &mut world.commands()));
        assert_eq!(jobs.parcels.len(), 2);
        assert_eq!(jobs.parcels[0].label, "Shared first address");
        assert!(matches!(jobs.parcels[0].state, ParcelState::Filed(_)));
        assert_eq!(jobs.progress()["score"], 0);
        assert_eq!(jobs.progress()["delivered"], 0);
        assert_eq!(jobs.hud_data(Vec3::ZERO)["delivered"], 1);
        assert_eq!(jobs.current_target(), Some(Vec3::new(30., 0., 40.)));
        assert!(!jobs.receive_route(2, 99, stops(), &mut world.commands()));
        let mut stale = stops();
        stale[0].completion = None;
        assert!(!jobs.receive_route(3, 6, stale, &mut world.commands()));
        assert!(matches!(jobs.parcels[0].state, ParcelState::Filed(_)));
    }
    #[test]
    fn the_host_confirms_only_one_winner_per_address_and_rejects_other_days_and_unknown_addresses()
    {
        let mut world = World::new();
        let mut jobs = Jobs::default();
        jobs.day = 1;
        jobs.configure_shared("host", true);
        jobs.parcels.push(Parcel {
            address: "way/1".into(),
            label: "First".into(),
            point: Vec3::ZERO,
            state: ParcelState::InVan,
            completion: None,
            dropped_at: None,
            prop: None,
        });
        assert!(!jobs.confirm_delivery(2, "way/1", receipt("alice"), &mut world.commands()));
        assert!(!jobs.confirm_delivery(1, "unknown", receipt("alice"), &mut world.commands()));
        assert!(jobs.confirm_delivery(1, "way/1", receipt("alice"), &mut world.commands()));
        assert!(!jobs.confirm_delivery(1, "way/1", receipt("bob"), &mut world.commands()));
        assert_eq!(jobs.parcels[0].completion.as_ref().unwrap().device, "alice");
        assert_eq!(jobs.progress()["score"], 0);
    }
    #[test]
    fn own_receipts_credit_once_and_new_days_reset_them() {
        let mut world = World::new();
        let mut jobs = Jobs::default();
        jobs.day = 1;
        jobs.configure_shared("host", false);
        assert!(jobs.receive_route(1, 7, stops(), &mut world.commands()));
        assert_eq!(jobs.progress()["score"], 100);
        assert!(!jobs.receive_route(1, 7, stops(), &mut world.commands()));
        let mut newer = stops();
        newer[0].completion = None;
        assert!(jobs.receive_route(1, 8, newer.clone(), &mut world.commands()));
        assert_eq!(jobs.progress()["score"], 100);
        jobs.day = 2;
        jobs.reset_shared_day();
        for p in &mut jobs.parcels {
            p.state = ParcelState::InVan;
        }
        assert!(jobs.receive_route(2, 1, newer, &mut world.commands()));
        assert_eq!(jobs.progress()["score"], 0);
        assert_eq!(jobs.hud_data(Vec3::ZERO)["delivered"], 0);
    }
    #[test]
    fn another_delivery_does_not_replace_the_parcel_in_your_hands() {
        let mut world = World::new();
        let mut jobs = Jobs::default();
        jobs.day = 1;
        jobs.configure_shared("guest", false);
        let mut initial = stops();
        initial[0].completion = None;
        jobs.receive_route(1, 1, initial, &mut world.commands());
        jobs.selected = 1;
        jobs.navigation = 0;
        jobs.parcels[1].state = ParcelState::Carrying;
        jobs.receive_route(1, 2, stops(), &mut world.commands());
        assert_eq!(jobs.selected, 1);
        assert_eq!(jobs.navigation, 1);
        assert_eq!(jobs.parcels[1].state, ParcelState::Carrying);
    }
}
