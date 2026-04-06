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

#[test]
fn pull_to_hand_distance_eases_to_target_distance() {
    let start_distance = 4.0;
    let target_distance = 2.5;
    let duration = 0.3;

    assert_eq!(
        pull_to_hand_distance(start_distance, target_distance, 0.0, duration),
        start_distance
    );
    assert!(
        (pull_to_hand_distance(start_distance, target_distance, duration, duration)
            - target_distance)
            .abs()
            < 0.0001
    );

    let halfway = pull_to_hand_distance(start_distance, target_distance, duration * 0.5, duration);
    assert!(halfway < start_distance);
    assert!(halfway > target_distance);
}

#[test]
fn pull_to_hand_arc_height_peaks_midway_and_returns_to_zero() {
    let duration = 0.4;
    let max_height = 0.35;

    assert_eq!(pull_to_hand_arc_height(0.0, duration, max_height), 0.0);
    assert!(
        (pull_to_hand_arc_height(duration * 0.5, duration, max_height) - max_height).abs() < 0.0001
    );
    assert!(pull_to_hand_arc_height(duration, duration, max_height).abs() < 0.0001);
}

#[test]
fn placement_frame_rotation_aligns_up_with_surface_normal() {
    let normal = Vec3::new(0.0, 0.70710677, 0.70710677).normalize();
    let rotation = placement_frame_rotation(normal, -Vec3::Z);

    let up = rotation * Vec3::Y;
    let forward = rotation * -Vec3::Z;

    assert!(
        up.distance(normal) < 0.0001,
        "expected up {normal:?}, got {up:?}"
    );
    assert!(forward.dot(normal).abs() < 0.0001);
}

#[test]
fn default_throw_profile_matches_lobbed_throw_formula() {
    let context = ThrowProfileContext {
        interactor: Entity::from_raw_u32(1).unwrap(),
        object: Entity::from_raw_u32(2).unwrap(),
        actor_forward: -Vec3::Z,
        actor_up: Vec3::Y,
        actor_right: Vec3::X,
        actor_velocity: Vec3::new(2.0, 0.5, 0.0),
        impulse_scale: 1.1,
        angular_impulse_scale: 0.75,
        config: crate::ThrowConfig {
            impulse: 16.0,
            angular_impulse: 2.4,
            upward_bias: 0.1,
            inherit_actor_velocity: true,
        },
        response: ThrowResponseOverride::default(),
    };

    let impulse = DefaultThrowProfile.impulses(&context);

    assert_eq!(
        impulse.linear,
        compute_throw_impulse(
            context.actor_forward,
            context.actor_velocity,
            context.config.impulse * context.impulse_scale,
            context.config.upward_bias,
            context.config.inherit_actor_velocity,
            &context.response,
        )
    );
    assert_eq!(
        impulse.angular,
        context.actor_right
            * context.config.angular_impulse
            * context.angular_impulse_scale
            * context.response.angular_impulse_scale
    );
}

#[test]
fn throw_profile_provider_supports_callbacks() {
    let provider = ThrowProfileProvider::from_callback(|context| ThrowImpulse {
        linear: context.actor_up * 3.0 + context.actor_velocity,
        angular: context.actor_forward * 2.0,
    });
    let context = ThrowProfileContext {
        interactor: Entity::from_raw_u32(1).unwrap(),
        object: Entity::from_raw_u32(2).unwrap(),
        actor_forward: -Vec3::Z,
        actor_up: Vec3::Y,
        actor_right: Vec3::X,
        actor_velocity: Vec3::new(1.0, 0.0, 0.0),
        impulse_scale: 1.0,
        angular_impulse_scale: 1.0,
        config: crate::ThrowConfig::default(),
        response: ThrowResponseOverride::default(),
    };

    let impulse = provider.impulses(&context);

    assert_eq!(impulse.linear, Vec3::new(1.0, 3.0, 0.0));
    assert_eq!(impulse.angular, Vec3::new(0.0, 0.0, -2.0));
}
