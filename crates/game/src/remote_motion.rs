//! Interpolate network poses behind the receive clock, with bounded prediction during jitter.
use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Clone, Copy)]
struct Sample {
    at: f32,
    position: Vec3,
    rotation: Quat,
    velocity: Vec3,
}
pub(crate) struct Motion {
    samples: VecDeque<Sample>,
    interval: f32,
    render_at: f32,
    sampled_at: f32,
}
impl Motion {
    pub fn new(now: f32, position: Vec3, rotation: Quat, velocity: Vec3) -> Self {
        Self {
            interval: 0.2,
            render_at: now - 0.2,
            sampled_at: now,
            samples: VecDeque::from([Sample {
                at: now,
                position,
                rotation,
                velocity,
            }]),
        }
    }
    pub fn push(&mut self, now: f32, position: Vec3, rotation: Quat, velocity: Vec3) {
        let last = self.samples.back().unwrap();
        if now <= last.at {
            return;
        }
        if last.position.distance(position) > 30. {
            self.samples.clear();
            self.interval = 0.2;
            self.render_at = now - 0.2;
            self.sampled_at = now;
        } else {
            // Track relay gaps quickly and recover latency slowly, avoiding repeated buffer underruns.
            self.interval = (now - last.at).clamp(0.1, 0.9).max(self.interval * 0.95);
        }
        self.samples.push_back(Sample {
            at: now,
            position,
            rotation,
            velocity,
        });
        while self.samples.len() > 16 {
            self.samples.pop_front();
        }
    }
    pub fn sample(&mut self, now: f32) -> Transform {
        let dt = (now - self.sampled_at).max(0.);
        self.sampled_at = now;
        let desired = now - self.interval.clamp(0.2, 0.9);
        // Adjust playback speed rather than jumping the render clock when latency changes.
        let speed = (1. + (desired - (self.render_at + dt)) * 2.).clamp(0.8, 1.2);
        self.render_at += dt * speed;
        let at = self.render_at;
        let first = self.samples.front().unwrap();
        if at <= first.at {
            return Transform::from_translation(first.position).with_rotation(first.rotation);
        }
        for (a, b) in self.samples.iter().zip(self.samples.iter().skip(1)) {
            if at <= b.at {
                let t = ((at - a.at) / (b.at - a.at)).clamp(0., 1.);
                return Transform::from_translation(a.position.lerp(b.position, t))
                    .with_rotation(a.rotation.slerp(b.rotation, t));
            }
        }
        let last = self.samples.back().unwrap();
        let age = (at - last.at).max(0.);
        // Predict through one observed relay interval, then stop; never extrapolate indefinitely.
        let horizon = self.interval.clamp(0.2, 0.8);
        let tail = (age - horizon).clamp(0., 0.2);
        let predicted = age.min(horizon) + tail - tail * tail / 0.4;
        Transform::from_translation(last.position + last.velocity * predicted)
            .with_rotation(last.rotation)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn walking_peer_keeps_moving_between_delayed_packets() {
        let mut motion = Motion::new(0., Vec3::ZERO, Quat::IDENTITY, Vec3::X * 4.);
        let mut previous = 0.;
        for frame in 1..=240 {
            let now = frame as f32 / 60.;
            if frame % 18 == 0 {
                motion.push(now, Vec3::X * 4. * now, Quat::IDENTITY, Vec3::X * 4.);
            }
            let x = motion.sample(now).translation.x;
            if frame > 60 {
                assert!(
                    (0.035..0.10).contains(&(x - previous)),
                    "frame {frame}: step {}",
                    x - previous
                );
            }
            previous = x;
        }
    }
    #[test]
    fn lost_connection_stops_prediction_and_teleports_reset_the_buffer() {
        let mut motion = Motion::new(0., Vec3::ZERO, Quat::IDENTITY, Vec3::X * 4.);
        assert_eq!(motion.sample(2.).translation, motion.sample(3.).translation);
        motion.push(4., Vec3::X * 200., Quat::IDENTITY, Vec3::ZERO);
        assert_eq!(motion.sample(4.).translation, Vec3::X * 200.);
    }
    #[test]
    fn irregular_slow_relay_keeps_the_render_clock_moving() {
        let mut motion = Motion::new(0., Vec3::ZERO, Quat::IDENTITY, Vec3::X * 4.);
        let intervals = [18, 54, 36, 42];
        let mut packet = 0;
        let mut next = intervals[0];
        let mut previous = 0.;
        for frame in 1..=900 {
            let now = frame as f32 / 60.;
            if frame == next {
                motion.push(now, Vec3::X * 4. * now, Quat::IDENTITY, Vec3::X * 4.);
                packet += 1;
                next += intervals[packet % intervals.len()];
            }
            let x = motion.sample(now).translation.x;
            if frame > 240 {
                assert!(
                    (0.025..0.12).contains(&(x - previous)),
                    "frame {frame}: step {}",
                    x - previous
                );
            }
            previous = x;
        }
    }
}
