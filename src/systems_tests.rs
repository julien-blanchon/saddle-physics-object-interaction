use std::time::Duration;

use avian3d::prelude::{
    Collider, CollisionLayers, LayerMask, LinearVelocity, Mass, PhysicsPlugins, RigidBody,
    TransformInterpolation,
};
use bevy::{asset::AssetApp, prelude::*, scene::ScenePlugin, time::TimeUpdateStrategy};

use crate::{
    AcquireFailureReason, AcquisitionMode, HoldDistance, HoldOrientationMode, InteractableBody,
    ObjectInteractionConfig, ObjectInteractionDiagnostics, ObjectInteractionPlugin,
    ObjectInteractor, ReleaseHeldObject, SetInteractionTarget, SetSurfacePlacementMode,
    SurfacePlacementMode, ThrowHeldObject, TryAcquireObject,
};

fn test_app(config: ObjectInteractionConfig) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(TransformPlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(ObjectInteractionPlugin::default().with_config(config));
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        1.0 / 60.0,
    )));
    app.init_asset::<Mesh>();
    app.finish();
    app
}

fn spawn_interactor(app: &mut App, max_mass: Option<f32>) -> Entity {
    spawn_interactor_with_mode(app, max_mass, AcquisitionMode::Hybrid)
}

fn spawn_interactor_with_mode(
    app: &mut App,
    max_mass: Option<f32>,
    acquisition_mode: AcquisitionMode,
) -> Entity {
    let transform =
        Transform::from_xyz(0.0, 1.1, 5.4).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    app.world_mut()
        .spawn((
            Name::new("Interactor"),
            ObjectInteractor {
                max_target_mass: max_mass,
                acquisition_mode,
                ..default()
            },
            HoldDistance(2.5),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            transform,
            GlobalTransform::IDENTITY,
        ))
        .id()
}

fn spawn_interactor_without_hold_distance(app: &mut App) -> Entity {
    let transform =
        Transform::from_xyz(0.0, 1.1, 5.4).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    app.world_mut()
        .spawn((
            Name::new("Interactor"),
            ObjectInteractor::default(),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            transform,
            GlobalTransform::IDENTITY,
        ))
        .id()
}

fn spawn_prop(
    app: &mut App,
    name: &str,
    position: Vec3,
    mass: f32,
    layers: Option<CollisionLayers>,
) -> Entity {
    let mut entity = app.world_mut().spawn((
        Name::new(name.to_owned()),
        InteractableBody::default(),
        RigidBody::Dynamic,
        Collider::cuboid(0.45, 0.45, 0.45),
        Mass(mass),
        TransformInterpolation,
        Transform::from_translation(position),
        GlobalTransform::IDENTITY,
        LinearVelocity::ZERO,
    ));
    if let Some(layers) = layers {
        entity.insert(layers);
    }
    entity.id()
}

fn spawn_wall(app: &mut App, position: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            Name::new("Wall"),
            RigidBody::Static,
            Collider::cuboid(0.35, 1.2, 0.9),
            Transform::from_translation(position),
            GlobalTransform::IDENTITY,
        ))
        .id()
}

fn settle_world(app: &mut App) {
    app.update();
    app.update();
}

fn run_frames(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

#[test]
fn acquires_a_valid_prop() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);
    let prop = spawn_prop(&mut app, "Crate", Vec3::new(0.0, 1.0, 0.0), 6.0, None);
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();

    assert_eq!(
        app.world()
            .get::<crate::Holding>(actor)
            .map(|holding| holding.0),
        Some(prop)
    );
    assert_eq!(
        app.world()
            .get::<crate::HeldBy>(prop)
            .map(|held_by| held_by.0),
        Some(actor)
    );
}

#[test]
fn rejects_an_overweight_prop() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, Some(20.0));
    spawn_prop(&mut app, "Heavy Coil", Vec3::new(0.0, 1.0, 0.0), 80.0, None);
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();

    assert!(app.world().get::<crate::Holding>(actor).is_none());
    let diagnostics = app.world().resource::<ObjectInteractionDiagnostics>();
    assert_eq!(
        diagnostics
            .last_failure
            .as_ref()
            .map(|failure| failure.reason),
        Some(AcquireFailureReason::TargetTooHeavy)
    );
}

#[test]
fn forced_release_occurs_when_actor_moves_too_far() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);
    let prop = spawn_prop(&mut app, "Crate", Vec3::new(0.0, 1.0, 0.0), 6.0, None);
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();
    assert!(app.world().get::<crate::Holding>(actor).is_some());

    let far_transform =
        Transform::from_xyz(8.0, 1.1, 9.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    app.world_mut()
        .entity_mut(actor)
        .insert((far_transform, GlobalTransform::from(far_transform)));

    run_frames(&mut app, 4);

    assert!(app.world().get::<crate::Holding>(actor).is_none());
    let diagnostics = app.world().resource::<ObjectInteractionDiagnostics>();
    assert_eq!(
        diagnostics
            .last_release
            .as_ref()
            .map(|release| release.reason),
        Some(crate::ReleaseReason::DistanceExceeded)
    );
    assert!(app.world().get::<crate::HeldBy>(prop).is_none());
}

#[test]
fn release_restores_collision_layers() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);
    let original_layers = CollisionLayers::new(0b0001, LayerMask::ALL);
    let prop = spawn_prop(
        &mut app,
        "Crate",
        Vec3::new(0.0, 1.0, 0.0),
        6.0,
        Some(original_layers),
    );
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();
    let held_layers = *app.world().get::<CollisionLayers>(prop).unwrap();
    assert_ne!(held_layers, original_layers);

    app.world_mut()
        .write_message(ReleaseHeldObject { interactor: actor });
    run_frames(&mut app, 4);

    assert_eq!(
        app.world().get::<CollisionLayers>(prop).copied(),
        Some(original_layers)
    );
}

#[test]
fn throw_applies_forward_impulse_and_clears_hold() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);
    let prop = spawn_prop(&mut app, "Crate", Vec3::new(0.0, 1.0, 0.0), 6.0, None);
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();
    assert!(app.world().get::<crate::Holding>(actor).is_some());

    app.world_mut().write_message(ThrowHeldObject {
        interactor: actor,
        impulse_scale: 1.0,
        angular_impulse_scale: 1.0,
    });
    run_frames(&mut app, 4);

    assert!(app.world().get::<crate::Holding>(actor).is_none());
    let velocity = app.world().get::<LinearVelocity>(prop).unwrap().0;
    assert!(
        velocity.z < -1.0,
        "expected forward throw, got velocity {velocity:?}"
    );
}

#[test]
fn forgiving_acquisition_modes_can_grab_off_axis_targets() {
    let off_axis_position = Vec3::new(0.95, 1.0, 0.0);

    let mut raycast_only = test_app(ObjectInteractionConfig::default());
    let raycast_actor =
        spawn_interactor_with_mode(&mut raycast_only, None, AcquisitionMode::RaycastOnly);
    spawn_prop(
        &mut raycast_only,
        "Offset Crate",
        off_axis_position,
        6.0,
        None,
    );
    settle_world(&mut raycast_only);

    raycast_only.world_mut().write_message(TryAcquireObject {
        interactor: raycast_actor,
    });
    raycast_only.update();

    assert!(
        raycast_only
            .world()
            .get::<crate::Holding>(raycast_actor)
            .is_none()
    );
    assert_eq!(
        raycast_only
            .world()
            .resource::<ObjectInteractionDiagnostics>()
            .last_failure
            .as_ref()
            .map(|failure| failure.reason),
        Some(AcquireFailureReason::NoValidTarget)
    );

    let mut hybrid = test_app(ObjectInteractionConfig::default());
    let hybrid_actor = spawn_interactor_with_mode(&mut hybrid, None, AcquisitionMode::Hybrid);
    let hybrid_prop = spawn_prop(&mut hybrid, "Offset Crate", off_axis_position, 6.0, None);
    settle_world(&mut hybrid);

    hybrid.world_mut().write_message(TryAcquireObject {
        interactor: hybrid_actor,
    });
    hybrid.update();

    assert_eq!(
        hybrid
            .world()
            .get::<crate::Holding>(hybrid_actor)
            .map(|holding| holding.0),
        Some(hybrid_prop)
    );

    let mut overlap_only = test_app(ObjectInteractionConfig::default());
    let overlap_actor =
        spawn_interactor_with_mode(&mut overlap_only, None, AcquisitionMode::OverlapOnly);
    let overlap_prop = spawn_prop(
        &mut overlap_only,
        "Offset Crate",
        off_axis_position,
        6.0,
        None,
    );
    settle_world(&mut overlap_only);

    overlap_only.world_mut().write_message(TryAcquireObject {
        interactor: overlap_actor,
    });
    overlap_only.update();

    assert_eq!(
        overlap_only
            .world()
            .get::<crate::Holding>(overlap_actor)
            .map(|holding| holding.0),
        Some(overlap_prop)
    );
}

#[test]
fn config_default_hold_distance_seeds_new_interactors() {
    let mut app = test_app(ObjectInteractionConfig {
        hold: crate::HoldConfig {
            default_distance: 3.4,
            ..default()
        },
        ..default()
    });
    let actor = spawn_interactor_without_hold_distance(&mut app);

    app.update();

    assert_eq!(
        app.world().get::<HoldDistance>(actor).map(|value| value.0),
        Some(3.4)
    );
}

#[test]
fn only_one_interactor_can_claim_a_prop_per_frame() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor_a = spawn_interactor(&mut app, None);
    let actor_b = spawn_interactor(&mut app, None);
    let prop = spawn_prop(
        &mut app,
        "Shared Crate",
        Vec3::new(0.0, 1.0, 0.0),
        6.0,
        None,
    );
    settle_world(&mut app);

    app.world_mut().write_message(TryAcquireObject {
        interactor: actor_a,
    });
    app.world_mut().write_message(TryAcquireObject {
        interactor: actor_b,
    });
    app.update();

    let holders = [actor_a, actor_b]
        .into_iter()
        .filter(|actor| {
            app.world()
                .get::<crate::Holding>(*actor)
                .is_some_and(|holding| holding.0 == prop)
        })
        .count();

    assert_eq!(
        holders, 1,
        "expected exactly one interactor to hold the prop"
    );
    assert!(app.world().get::<crate::HeldBy>(prop).is_some());
    assert_eq!(
        app.world()
            .resource::<ObjectInteractionDiagnostics>()
            .last_failure
            .as_ref()
            .map(|failure| failure.reason),
        Some(AcquireFailureReason::TargetAlreadyHeld)
    );
}

#[test]
fn explicit_target_reports_blocked_when_occluded() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);
    let prop = spawn_prop(
        &mut app,
        "Occluded Crate",
        Vec3::new(0.0, 1.0, 0.0),
        6.0,
        None,
    );
    spawn_wall(&mut app, Vec3::new(0.0, 1.0, 2.3));
    settle_world(&mut app);

    app.world_mut().write_message(SetInteractionTarget {
        interactor: actor,
        target: Some(prop),
    });
    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();

    assert!(app.world().get::<crate::Holding>(actor).is_none());
    assert_eq!(
        app.world()
            .resource::<ObjectInteractionDiagnostics>()
            .last_failure
            .as_ref()
            .map(|failure| failure.reason),
        Some(AcquireFailureReason::TargetBlocked)
    );
}

#[test]
fn default_orientation_mode_comes_from_config_when_interactor_uses_defaults() {
    let mut app = test_app(ObjectInteractionConfig {
        hold: crate::HoldConfig {
            orientation_mode: HoldOrientationMode::AlignToInteractor,
            ..default()
        },
        ..default()
    });
    let actor = spawn_interactor_without_hold_distance(&mut app);
    let prop = spawn_prop(
        &mut app,
        "Aligned Crate",
        Vec3::new(0.0, 1.0, 0.0),
        6.0,
        None,
    );
    app.world_mut().entity_mut(prop).insert((
        Transform::from_translation(Vec3::new(0.0, 1.0, 0.0))
            .with_rotation(Quat::from_rotation_y(1.2)),
        GlobalTransform::IDENTITY,
    ));
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();

    let runtime = app
        .world()
        .get::<crate::components::HeldRuntime>(prop)
        .unwrap();
    assert_eq!(runtime.base_rotation_offset, Quat::IDENTITY);
}

#[test]
fn set_surface_placement_mode_message_toggles_interactor_flag() {
    let mut app = test_app(ObjectInteractionConfig::default());
    let actor = spawn_interactor(&mut app, None);

    app.world_mut().write_message(SetSurfacePlacementMode {
        interactor: actor,
        enabled: true,
    });
    app.update();

    assert_eq!(
        app.world()
            .get::<SurfacePlacementMode>(actor)
            .map(|mode| mode.enabled),
        Some(true)
    );

    app.world_mut().write_message(SetSurfacePlacementMode {
        interactor: actor,
        enabled: false,
    });
    app.update();

    assert_eq!(
        app.world()
            .get::<SurfacePlacementMode>(actor)
            .map(|mode| mode.enabled),
        Some(false)
    );
}

#[test]
fn acquisition_seeds_pull_to_hand_runtime_from_config() {
    let mut app = test_app(ObjectInteractionConfig {
        hold: crate::HoldConfig {
            default_distance: 2.25,
            pull_to_hand: crate::PullToHandConfig {
                enabled: true,
                duration_seconds: 0.45,
                arc_height: 0.2,
                min_start_distance: 0.4,
            },
            ..default()
        },
        ..default()
    });
    let actor = spawn_interactor_without_hold_distance(&mut app);
    let prop = spawn_prop(&mut app, "Crate", Vec3::new(0.0, 1.0, 1.2), 6.0, None);
    settle_world(&mut app);

    app.world_mut()
        .write_message(TryAcquireObject { interactor: actor });
    app.update();

    let runtime = app
        .world()
        .get::<crate::components::HeldRuntime>(prop)
        .copied()
        .expect("expected acquired prop runtime");

    assert_eq!(runtime.pull_elapsed, 0.0);
    assert!((runtime.pull_duration - 0.45).abs() < 0.0001);
    assert!((runtime.pull_target_distance - 2.25).abs() < 0.0001);
    assert!(runtime.pull_start_distance >= runtime.pull_target_distance);
}
