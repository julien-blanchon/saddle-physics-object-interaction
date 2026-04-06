//! Pick up objects at close range and rotate them for inspection.
//!
//! Walk around with **WASD**, look with **mouse**. **Shift** to sprint.
//! **E** or **LMB** to acquire, **Q/C** to rotate, **R** or **RMB** to release,
//! **F** or **LMB** (while holding) to throw.
//! Short hold distance and aligned orientation keep props centered in view.

use avian3d::prelude::{
    AngularDamping, Collider, CollisionLayers, LayerMask, LinearDamping, Mass, PhysicsPlugins,
    RigidBody, TransformInterpolation,
};
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use bevy_enhanced_input::prelude::*;
use bevy_flair::FlairPlugin;
use bevy_input_focus::{InputDispatchPlugin, tab_navigation::TabNavigationPlugin};
use bevy_ui_widgets::UiWidgetsPlugins;
use saddle_pane::prelude::*;
use saddle_physics_object_interaction::{
    AdjustHoldDistance, CycleDirection, CycleInteractionTarget, DefaultSelectionScorer,
    DefaultThrowProfile, HeldBy, HoldDistance, HoldOrientationMode, HoldOrientationOverride,
    HoldPointOverride, InteractableBody, InteractionCollisionPolicy, InteractionTarget,
    ObjectInteractionConfig, ObjectInteractionDebugSettings, ObjectInteractionPlugin,
    ObjectInteractionState, ObjectInteractionSystems, ObjectInteractor, PreferredHoldDistance,
    ReleaseHeldObject, RotateHeldObject, SelectionScorerProvider, ThrowHeldObject,
    ThrowProfileProvider, ThrowResponseOverride, TryAcquireObject,
};

// ---------------------------------------------------------------------------
// Input actions (keyboard interaction — NOT movement)
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
// FPS controller
// ---------------------------------------------------------------------------

#[derive(Component)]
struct FpsController {
    yaw: f32,
    pitch: f32,
    speed: f32,
    sensitivity: f32,
}

#[derive(Resource)]
struct CursorGrabbed(bool);

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
    app.insert_resource(SelectionScorerProvider::from_scorer(
        DefaultSelectionScorer,
    ));
    app.insert_resource(ThrowProfileProvider::from_profile(DefaultThrowProfile));
    app.insert_resource(ObjectInteractionDebugSettings {
        enabled: true,
        draw_gizmos: false,
    });
    app.insert_resource(CursorGrabbed(true));

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

    app.add_systems(Startup, (setup_scene, grab_cursor));
    app.add_systems(
        Update,
        (
            handle_cursor,
            fps_look,
            fps_move,
            mouse_interact,
            scroll_distance,
        )
            .before(ObjectInteractionSystems::ReadCommands),
    );
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
// Cursor grab
// ---------------------------------------------------------------------------

fn grab_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.grab_mode = CursorGrabMode::Locked;
    cursor.visible = false;
}

fn handle_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut grabbed: ResMut<CursorGrabbed>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        grabbed.0 = false;
    }
    if !grabbed.0
        && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
    {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        grabbed.0 = true;
    }
}

// ---------------------------------------------------------------------------
// FPS look & movement
// ---------------------------------------------------------------------------

fn fps_look(
    grabbed: Res<CursorGrabbed>,
    mut motion: MessageReader<MouseMotion>,
    mut q: Query<(&mut FpsController, &mut Transform), With<Interactor>>,
) {
    if !grabbed.0 {
        motion.clear();
        return;
    }
    let Ok((mut ctrl, mut transform)) = q.single_mut() else {
        motion.clear();
        return;
    };
    for event in motion.read() {
        ctrl.yaw -= event.delta.x * ctrl.sensitivity;
        ctrl.pitch = (ctrl.pitch - event.delta.y * ctrl.sensitivity)
            .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
    }
    transform.rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);
}

fn fps_move(
    grabbed: Res<CursorGrabbed>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q: Query<(&FpsController, &mut Transform), With<Interactor>>,
) {
    if !grabbed.0 {
        return;
    }
    let Ok((ctrl, mut transform)) = q.single_mut() else {
        return;
    };
    let mut direction = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction += *transform.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        direction += *transform.back();
    }
    if keys.pressed(KeyCode::KeyA) {
        direction += *transform.left();
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += *transform.right();
    }
    direction.y = 0.0;
    if let Some(dir) = direction.try_normalize() {
        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            ctrl.speed * 2.0
        } else {
            ctrl.speed
        };
        transform.translation += dir * speed * time.delta_secs();
    }
}

// ---------------------------------------------------------------------------
// Mouse-based interaction
// ---------------------------------------------------------------------------

fn mouse_interact(
    grabbed: Res<CursorGrabbed>,
    mouse: Res<ButtonInput<MouseButton>>,
    q: Query<(Entity, &ObjectInteractionState), With<Interactor>>,
    mut acquire: MessageWriter<TryAcquireObject>,
    mut throw: MessageWriter<ThrowHeldObject>,
    mut release: MessageWriter<ReleaseHeldObject>,
) {
    if !grabbed.0 {
        return;
    }
    let Ok((interactor, state)) = q.single() else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        match *state {
            ObjectInteractionState::Holding(_) => {
                throw.write(ThrowHeldObject {
                    interactor,
                    impulse_scale: 1.0,
                    angular_impulse_scale: 1.0,
                });
            }
            _ => {
                acquire.write(TryAcquireObject { interactor });
            }
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        release.write(ReleaseHeldObject { interactor });
    }
}

fn scroll_distance(
    grabbed: Res<CursorGrabbed>,
    mut scroll: MessageReader<MouseWheel>,
    q: Query<Entity, With<Interactor>>,
    mut w: MessageWriter<AdjustHoldDistance>,
) {
    if !grabbed.0 {
        scroll.clear();
        return;
    }
    let Ok(interactor) = q.single() else {
        scroll.clear();
        return;
    };
    for event in scroll.read() {
        w.write(AdjustHoldDistance {
            interactor,
            delta: event.y * 0.3,
        });
    }
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

    // -- Interactor (FPS player) facing the inspect prism -----------------------
    let start_pos = Vec3::new(1.75, 1.45, 4.55);
    let look_at = Vec3::new(2.4, 0.9, -0.05);
    let initial_transform = Transform::from_translation(start_pos).looking_at(look_at, Vec3::Y);
    let (yaw, pitch, _) = initial_transform.rotation.to_euler(EulerRot::YXZ);

    commands
        .spawn((
            Name::new("Interactor"),
            Interactor,
            InputCtx,
            FpsController {
                yaw,
                pitch,
                speed: 4.0,
                sensitivity: 0.002,
            },
            ObjectInteractor {
                max_target_mass: Some(45.0),
                ..default()
            },
            HoldDistance(1.2),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            initial_transform,
            GlobalTransform::IDENTITY,
            Visibility::Visible,
            actions!(InputCtx[
                (Action::<AcquireAction>::new(), bindings![KeyCode::KeyE]),
                (Action::<ReleaseAction>::new(), bindings![KeyCode::KeyR]),
                (Action::<ThrowAction>::new(), bindings![KeyCode::KeyF]),
                (Action::<NearAction>::new(), bindings![KeyCode::KeyZ]),
                (Action::<FarAction>::new(), bindings![KeyCode::KeyX]),
                (Action::<RotateLeftAction>::new(), bindings![KeyCode::KeyQ]),
                (Action::<RotateRightAction>::new(), bindings![KeyCode::KeyC]),
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
             WASD move | Mouse look | Shift sprint | Esc release cursor\n\
             LMB grab/throw | RMB drop | Scroll distance\n\
             E grab | Q/C rotate | R drop | F throw | Z/X distance | Tab cycle\n\
             Short hold distance, aligned orientation, close-up rotation for inspection.",
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            width: px(600.0),
            ..default()
        },
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));

    // Crosshair
    commands.spawn((
        Name::new("Crosshair"),
        Text::new("+"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
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
// Input observers (keyboard)
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
        let color =
            if held == Some(entity) || held_by.is_some_and(|hb| q_interactor.get(hb.0).is_ok()) {
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
