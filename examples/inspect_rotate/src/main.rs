//! Pick up objects at close range and rotate them for inspection.
//!
//! Press **E** to acquire, **A/D** to rotate, **R** to release, **F** to throw.
//! Short hold distance and aligned orientation keep props centered in view.

use avian3d::prelude::{
    AngularDamping, Collider, CollisionLayers, LayerMask, LinearDamping, Mass, PhysicsPlugins,
    RigidBody, TransformInterpolation,
};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_flair::FlairPlugin;
use bevy_input_focus::{InputDispatchPlugin, tab_navigation::TabNavigationPlugin};
use bevy_ui_widgets::UiWidgetsPlugins;
use saddle_pane::prelude::*;
use saddle_physics_object_interaction::{
    AdjustHoldDistance, CycleDirection, CycleInteractionTarget, HeldBy, HoldDistance,
    HoldOrientationMode, HoldOrientationOverride, HoldPointOverride, InteractableBody,
    InteractionCollisionPolicy, InteractionTarget, ObjectInteractionConfig,
    ObjectInteractionDebugSettings, ObjectInteractionPlugin, ObjectInteractionState,
    ObjectInteractionSystems, ObjectInteractor, PreferredHoldDistance, ReleaseHeldObject,
    RotateHeldObject, ThrowHeldObject, ThrowResponseOverride, TryAcquireObject,
};

// ---------------------------------------------------------------------------
// Input actions
// ---------------------------------------------------------------------------

#[derive(InputAction)]
#[action_output(bool)]
struct AcquireAction;

#[derive(InputAction)]
#[action_output(bool)]
struct ReleaseAction;

#[derive(InputAction)]
#[action_output(bool)]
struct ThrowAction;

#[derive(InputAction)]
#[action_output(bool)]
struct NearAction;

#[derive(InputAction)]
#[action_output(bool)]
struct FarAction;

#[derive(InputAction)]
#[action_output(bool)]
struct RotateLeftAction;

#[derive(InputAction)]
#[action_output(bool)]
struct RotateRightAction;

#[derive(InputAction)]
#[action_output(bool)]
struct CycleAction;

#[derive(Component)]
struct InputCtx;

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Interactor;

#[derive(Component)]
struct PropVisual {
    base_color: Color,
}

#[derive(Component)]
struct Prop;

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // Inspect-rotate tuning: short hold distance, aligned orientation, disabled collision
    let config = ObjectInteractionConfig {
        acquisition: saddle_physics_object_interaction::AcquisitionConfig {
            max_distance: 5.8,
            forgiving_radius: 0.9,
            ..default()
        },
        hold: saddle_physics_object_interaction::HoldConfig {
            min_distance: 0.8,
            default_distance: 1.2,
            max_distance: 2.2,
            linear_stiffness: 180.0,
            linear_damping: 34.0,
            angular_stiffness: 82.0,
            angular_damping: 16.0,
            collision_policy: InteractionCollisionPolicy::DisableAll,
            orientation_mode: HoldOrientationMode::AlignToInteractor,
            ..default()
        },
        throw: saddle_physics_object_interaction::ThrowConfig {
            impulse: 9.0,
            angular_impulse: 1.4,
            ..default()
        },
        ..default()
    };

    let mut app = App::new();

    app.insert_resource(ClearColor(Color::srgb(0.04, 0.045, 0.055)));
    app.insert_resource(config.clone());
    app.insert_resource(ObjectInteractionDebugSettings {
        enabled: true,
        draw_gizmos: false,
    });

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "object_interaction / inspect_rotate".into(),
            resolution: (1440, 900).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins((
        FlairPlugin,
        InputDispatchPlugin,
        UiWidgetsPlugins,
        TabNavigationPlugin,
        PanePlugin,
    ));
    app.add_plugins(PhysicsPlugins::default());
    app.add_plugins(EnhancedInputPlugin);
    app.add_input_context::<InputCtx>();
    app.add_plugins(ObjectInteractionPlugin::default());

    app.add_systems(Startup, setup_scene);
    app.add_observer(on_acquire);
    app.add_observer(on_release);
    app.add_observer(on_throw);
    app.add_observer(on_near);
    app.add_observer(on_far);
    app.add_observer(on_rotate_left);
    app.add_observer(on_rotate_right);
    app.add_observer(on_cycle);
    app.add_systems(
        Update,
        tint_props.after(ObjectInteractionSystems::Presentation),
    );

    app.run();
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Lights
    commands.spawn((
        Name::new("Point Light"),
        PointLight {
            intensity: 2_200_000.0,
            range: 24.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(5.5, 8.0, 6.0),
    ));
    commands.spawn((
        Name::new("Fill Light"),
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-5.0, 8.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ground
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(24.0, 24.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.13, 0.12),
            perceptual_roughness: 1.0,
            ..default()
        })),
        RigidBody::Static,
        Collider::cuboid(12.0, 0.1, 12.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    // Backdrop
    commands.spawn((
        Name::new("Backdrop"),
        Mesh3d(meshes.add(Cuboid::new(12.0, 5.0, 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.08, 0.1),
            perceptual_roughness: 0.95,
            ..default()
        })),
        RigidBody::Static,
        Collider::cuboid(6.0, 2.5, 0.1),
        Transform::from_xyz(0.0, 2.0, -4.8),
    ));

    // -- Interactor facing the inspect prism ---------------------------------
    commands
        .spawn((
            Name::new("Interactor"),
            Interactor,
            InputCtx,
            ObjectInteractor {
                max_target_mass: Some(45.0),
                ..default()
            },
            HoldDistance(1.2),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            // Start at the Inspect station — looking at the prism
            Transform::from_xyz(1.75, 1.45, 4.55)
                .looking_at(Vec3::new(2.4, 0.9, -0.05), Vec3::Y),
            GlobalTransform::IDENTITY,
            Visibility::Visible,
            actions!(InputCtx[
                (Action::<AcquireAction>::new(), bindings![KeyCode::KeyE]),
                (Action::<ReleaseAction>::new(), bindings![KeyCode::KeyR]),
                (Action::<ThrowAction>::new(), bindings![KeyCode::KeyF, MouseButton::Left]),
                (Action::<NearAction>::new(), bindings![KeyCode::KeyZ]),
                (Action::<FarAction>::new(), bindings![KeyCode::KeyX]),
                (Action::<RotateLeftAction>::new(), bindings![KeyCode::KeyA]),
                (Action::<RotateRightAction>::new(), bindings![KeyCode::KeyD]),
                (Action::<CycleAction>::new(), bindings![KeyCode::Tab]),
            ]),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Camera"),
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        });

    // -- Props: objects designed for close-up inspection ----------------------

    // Inspect Prism -- aligned orientation, disabled collision, close hold
    spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        PropDef {
            name: "Inspect Prism",
            mesh: Sphere::new(0.45).mesh().ico(5).expect("ico sphere"),
            collider: Collider::sphere(0.45),
            position: Vec3::new(2.4, 0.85, -0.05),
            base_color: Color::srgb(0.82, 0.82, 0.90),
            mass: 3.0,
            extras: (
                PreferredHoldDistance(1.15),
                HoldOrientationOverride {
                    mode: HoldOrientationMode::AlignToInteractor,
                },
                HoldPointOverride {
                    local_offset: Vec3::ZERO,
                    local_rotation: Quat::IDENTITY,
                },
                InteractionCollisionPolicy::DisableAll,
                ThrowResponseOverride {
                    impulse_scale: 0.7,
                    angular_impulse_scale: 0.35,
                    inherit_actor_velocity: Some(false),
                    upward_bias_scale: 0.3,
                },
            ),
        },
    );

    // A crate for contrast -- behaves differently under inspect config
    spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        PropDef {
            name: "Light Crate",
            mesh: Mesh::from(Cuboid::new(0.9, 0.9, 0.9)),
            collider: Collider::cuboid(0.45, 0.45, 0.45),
            position: Vec3::new(0.0, 0.75, 0.0),
            base_color: Color::srgb(0.76, 0.52, 0.24),
            mass: 8.0,
            extras: (PreferredHoldDistance(1.2), ThrowResponseOverride::default()),
        },
    );

    // HUD
    commands.spawn((
        Name::new("HUD"),
        Text::new(
            "object_interaction / inspect_rotate\n\
             Short hold distance, aligned orientation, close-up rotation for inspection flow.\n\
             E acquire | A/D rotate | R release | F throw | Z/X distance | Tab cycle",
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            width: px(560.0),
            ..default()
        },
        TextFont {
            font_size: 17.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

// ---------------------------------------------------------------------------
// Prop spawning helper
// ---------------------------------------------------------------------------

struct PropDef<B: Bundle> {
    name: &'static str,
    mesh: Mesh,
    collider: Collider,
    position: Vec3,
    base_color: Color,
    mass: f32,
    extras: B,
}

fn spawn_prop<B: Bundle>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    spec: PropDef<B>,
) -> Entity {
    let mut entity = commands.spawn((
        Name::new(spec.name),
        Prop,
        PropVisual {
            base_color: spec.base_color,
        },
        InteractableBody::default(),
        Mesh3d(meshes.add(spec.mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: spec.base_color,
            perceptual_roughness: 0.68,
            metallic: 0.04,
            ..default()
        })),
        RigidBody::Dynamic,
        spec.collider,
        Mass(spec.mass),
        LinearDamping(0.32),
        AngularDamping(0.88),
        TransformInterpolation,
        CollisionLayers::new(0b0001, LayerMask::ALL),
        Transform::from_translation(spec.position),
        GlobalTransform::IDENTITY,
    ));
    entity.insert(spec.extras);
    entity.id()
}

// ---------------------------------------------------------------------------
// Input observers
// ---------------------------------------------------------------------------

fn on_acquire(trigger: On<Start<AcquireAction>>, mut w: MessageWriter<TryAcquireObject>) {
    w.write(TryAcquireObject {
        interactor: trigger.context,
    });
}

fn on_release(trigger: On<Start<ReleaseAction>>, mut w: MessageWriter<ReleaseHeldObject>) {
    w.write(ReleaseHeldObject {
        interactor: trigger.context,
    });
}

fn on_throw(trigger: On<Start<ThrowAction>>, mut w: MessageWriter<ThrowHeldObject>) {
    w.write(ThrowHeldObject {
        interactor: trigger.context,
        impulse_scale: 1.0,
        angular_impulse_scale: 1.0,
    });
}

fn on_near(trigger: On<Start<NearAction>>, mut w: MessageWriter<AdjustHoldDistance>) {
    w.write(AdjustHoldDistance {
        interactor: trigger.context,
        delta: -0.25,
    });
}

fn on_far(trigger: On<Start<FarAction>>, mut w: MessageWriter<AdjustHoldDistance>) {
    w.write(AdjustHoldDistance {
        interactor: trigger.context,
        delta: 0.25,
    });
}

fn on_rotate_left(trigger: On<Start<RotateLeftAction>>, mut w: MessageWriter<RotateHeldObject>) {
    w.write(RotateHeldObject {
        interactor: trigger.context,
        delta: Quat::from_rotation_y(18.0_f32.to_radians()),
    });
}

fn on_rotate_right(trigger: On<Start<RotateRightAction>>, mut w: MessageWriter<RotateHeldObject>) {
    w.write(RotateHeldObject {
        interactor: trigger.context,
        delta: Quat::from_rotation_y((-18.0_f32).to_radians()),
    });
}

fn on_cycle(trigger: On<Start<CycleAction>>, mut w: MessageWriter<CycleInteractionTarget>) {
    w.write(CycleInteractionTarget {
        interactor: trigger.context,
        direction: CycleDirection::Next,
    });
}

// ---------------------------------------------------------------------------
// Visual feedback
// ---------------------------------------------------------------------------

fn tint_props(
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_interactor: Query<(&InteractionTarget, &ObjectInteractionState), With<Interactor>>,
    q_props: Query<
        (
            Entity,
            &PropVisual,
            &MeshMaterial3d<StandardMaterial>,
            Option<&HeldBy>,
        ),
        With<Prop>,
    >,
) {
    let Ok((target, state)) = q_interactor.single() else {
        return;
    };
    let targeted = target.entity;
    let held = match *state {
        ObjectInteractionState::Holding(e) => Some(e),
        _ => None,
    };

    for (entity, visual, mat, held_by) in &q_props {
        let color = if held == Some(entity)
            || held_by.is_some_and(|hb| q_interactor.get(hb.0).is_ok())
        {
            Color::srgb(0.28, 0.96, 0.58)
        } else if targeted == Some(entity) {
            Color::srgb(0.98, 0.80, 0.24)
        } else {
            visual.base_color
        };
        if let Some(material) = materials.get_mut(&mat.0) {
            material.base_color = color;
        }
    }
}
