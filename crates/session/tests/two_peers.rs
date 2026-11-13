use std::thread;
use std::time::{Duration, Instant};

use paket_session::{Fact, Origin, host, invite_link, invite_origin, join};
use serde_json::json;

fn origin() -> Origin {
    Origin {
        lat: 52.4895,
        lon: 13.3908,
    }
}

#[test]
fn two_local_peers_share_delivery_and_pose() {
    let origin = origin();
    let host = host(origin).expect("host");
    let link = invite_link(&host.invitation);
    assert_eq!(invite_origin(&link).unwrap(), origin);
    let guest = join(&link).expect("guest");
    let mut forged: serde_json::Value = serde_json::from_str(&host.invitation).unwrap();
    forged["origin"] = json!({"lat": 1.0, "lon": 2.0});
    let outsider = join(&forged.to_string()).expect("outsider");

    host.publish(Fact::Delivery {
        origin,
        building_id: "way/201".into(),
    })
    .unwrap();
    host.publish(Fact::Pose {
        origin,
        device: host.device.clone(),
        east: 4.0,
        north: 5.0,
        heading: 0.25,
        up: 0.7,
        velocity_east: 0.0,
        velocity_north: 0.0,
        sequence: 1,
    })
    .unwrap();
    assert!(
        host.publish(Fact::Delivery {
            origin: Origin { lat: 1.0, lon: 2.0 },
            building_id: "way/9".into(),
        })
        .is_err()
    );

    let start = Instant::now();
    let mut delivery = false;
    let mut pose = false;
    let mut leaked = false;
    while start.elapsed() < Duration::from_secs(8) && !(delivery && pose) {
        for fact in guest.poll() {
            match fact {
                Fact::Delivery {
                    origin: got,
                    building_id,
                } => {
                    assert_eq!(got, origin);
                    assert_eq!(building_id, "way/201");
                    delivery = true;
                }
                Fact::Pose {
                    east,
                    north,
                    heading,
                    ..
                } => {
                    if (east - 4.0).abs() < 0.01
                        && (north - 5.0).abs() < 0.01
                        && (heading - 0.25).abs() < 0.01
                    {
                        pose = true;
                    }
                }
                _ => {}
            }
        }
        leaked |= outsider
            .poll()
            .into_iter()
            .any(|fact| fact.origin() == origin);
        thread::sleep(Duration::from_millis(40));
    }
    assert!(delivery, "guest did not see the delivery");
    assert!(pose, "guest did not see the bicycle");
    assert!(!leaked, "a different origin applied the session");
}

#[test]
fn delivery_arrives_after_a_burst_of_poses() {
    let origin = origin();
    let host = host(origin).expect("host");
    let guest = join(&host.invitation).expect("guest");
    for step in 0..40 {
        host.publish(Fact::Pose {
            origin,
            device: host.device.clone(),
            east: step as f32,
            north: 1.0,
            heading: 0.0,
            up: 0.7,
            velocity_east: 0.0,
            velocity_north: 0.0,
            sequence: 1,
        })
        .unwrap();
    }
    host.publish(Fact::Delivery {
        origin,
        building_id: "way/617568445".into(),
    })
    .unwrap();
    let start = Instant::now();
    let mut seen = false;
    while start.elapsed() < Duration::from_secs(4) {
        seen |= guest.poll().into_iter().any(|fact| {
            matches!(fact, Fact::Delivery { building_id, .. } if building_id == "way/617568445")
        });
        if seen {
            break;
        }
        thread::sleep(Duration::from_millis(40));
    }
    assert!(seen, "guest did not see the delivery behind the pose burst");
}
