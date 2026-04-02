use bevy::prelude::*;

use super::*;
use crate::{ThrowResponseOverride, config::ObjectInteractionConfig};

#[test]
fn spring_force_is_clamped() {
    let force = compute_spring_force(Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, 100.0, 0.0, 25.0);

    assert!((force.length() - 25.0).abs() < 0.001);
}

#[test]
fn align_torque_is_zero_when_rotation_matches() {
    let torque = compute_align_torque(
        Quat::IDENTITY,
        Quat::IDENTITY,
        Vec3::ZERO,
        64.0,
        12.0,
        180.0,
    );

    assert_eq!(torque, Vec3::ZERO);
}

#[test]
fn throw_impulse_adds_upward_bias_and_actor_velocity() {
    let impulse = compute_throw_impulse(
        -Vec3::Z,
        Vec3::new(2.0, 0.5, 0.0),
        16.0,
        0.1,
        true,
        &ThrowResponseOverride::default(),
    );

    assert!(impulse.z < -10.0);
    assert!(impulse.y > 1.5);
    assert!(impulse.x > 1.5);
}

#[test]
fn release_evaluation_prioritizes_break_distance() {
    let release = evaluate_release(5.0, 1.0, 1.0, &ObjectInteractionConfig::default());

    assert_eq!(release.reason, Some(crate::ReleaseReason::DistanceExceeded));
    assert!(!release.became_unstable);
}
