//! Mesh-picking integration: click a prop to target and acquire it.
//!
//! Walk around with **WASD**, look with **mouse**. **Shift** to sprint.
//! **Click** on a prop to set it as target and immediately grab it.
//! **E** also grabs nearest, **R** or **RMB** to release, **F** to throw,
//! **Q/C** rotate, **Scroll** or **Z/X** distance, **Tab** cycle.

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
    AdjustHoldDistance, CycleDirection, CycleInteractionTarget, HeldBy, HoldDistance,
    InteractableBody, InteractionTarget, ObjectInteractionConfig, ObjectInteractionDebugSettings,
    ObjectInteractionPlugin, ObjectInteractionState, ObjectInteractionSystems, ObjectInteractor,
    PreferredHoldDistance, ReleaseHeldObject, RotateHeldObject, SetInteractionTarget,
    ThrowHeldObject, ThrowResponseOverride, TryAcquireObject,
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
    let config = ObjectInteractionConfig {
        acquisition: saddle_physics_object_interaction::AcquisitionConfig {
            max_distance: 6.8,
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
    app.insert_resource(CursorGrabbed(false));

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "object_interaction / picking_integration".into(),
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

    // Enable mesh picking so clicks on 3D meshes fire Pointer<Click> events
    app.add_plugins(MeshPickingPlugin);

    app.add_plugins(ObjectInteractionPlugin::default());

    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (handle_cursor, fps_look, fps_move, scroll_distance)
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
// Cursor grab (picking mode: start ungrabbed so user can click props)
// ---------------------------------------------------------------------------

fn handle_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut grabbed: ResMut<CursorGrabbed>,
) {
    // RMB toggles FPS mode on/off
    if mouse.just_pressed(MouseButton::Right) {
        if grabbed.0 {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
            grabbed.0 = false;
        } else {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
            grabbed.0 = true;
        }
    }
    if keys.just_pressed(KeyCode::Escape) && grabbed.0 {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        grabbed.0 = false;
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

    // Ground (not pickable)
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
        Pickable::IGNORE,
    ));

    // Backdrop (not pickable)
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
        Pickable::IGNORE,
    ));

    // -- Interactor (FPS player) ------------------------------------------------
    let start_pos = Vec3::new(0.0, 1.45, 5.4);
    let look_at = Vec3::new(0.0, 0.9, 0.0);
    let initial_transform = Transform::from_translation(start_pos).looking_at(look_at, Vec3::Y);
    let (yaw, pitch, _) = initial_transform.rotation.to_euler(EulerRot::YXZ);

    let interactor = commands
        .spawn((
            Name::new("Interactor"),
            Interactor,
            InputCtx,
            FpsController {
                yaw,
                pitch,
                speed: 5.0,
                sensitivity: 0.002,
            },
            ObjectInteractor {
                max_target_mass: Some(45.0),
                ..default()
            },
            HoldDistance(2.0),
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
        })
        .id();

    // -- Props (pickable — clicking sets target + acquires) -------------------

    spawn_pickable_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        PropDef {
            name: "Light Crate",
            mesh: Mesh::from(Cuboid::new(0.9, 0.9, 0.9)),
            collider: Collider::cuboid(0.45, 0.45, 0.45),
            position: Vec3::new(0.0, 0.75, 0.0),
            base_color: Color::srgb(0.76, 0.52, 0.24),
            mass: 8.0,
            extras: (PreferredHoldDistance(2.4), ThrowResponseOverride::default()),
        },
    );

    spawn_pickable_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        PropDef {
            name: "Inspect Prism",
            mesh: Sphere::new(0.45).mesh().ico(5).expect("ico sphere"),
            collider: Collider::sphere(0.45),
            position: Vec3::new(2.4, 0.85, -0.05),
            base_color: Color::srgb(0.82, 0.82, 0.90),
            mass: 3.0,
            extras: (),
        },
    );

    spawn_pickable_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        PropDef {
            name: "Small Cylinder",
            mesh: Mesh::from(Cylinder::new(0.3, 0.6)),
            collider: Collider::cylinder(0.3, 0.6),
            position: Vec3::new(-1.5, 0.6, 0.8),
            base_color: Color::srgb(0.65, 0.35, 0.55),
            mass: 5.0,
            extras: (),
        },
    );

    // HUD
    commands.spawn((
        Name::new("HUD"),
        Text::new(
            "object_interaction / picking_integration\n\
             Click a prop to target and acquire it. RMB to toggle FPS mode.\n\
             WASD move (FPS mode) | Mouse look (FPS mode) | Esc release cursor\n\
             LMB click prop | E grab nearest | R release | F throw\n\
             Q/C rotate | Scroll distance | Z/X distance | Tab cycle",
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
}

// ---------------------------------------------------------------------------
// Prop spawning helpers
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

fn spawn_pickable_prop<B: Bundle>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    interactor: Entity,
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

    // Click-to-acquire: set the clicked entity as the explicit target, then acquire
    entity.observe(
        move |click: On<Pointer<Click>>,
              mut set_target: MessageWriter<SetInteractionTarget>,
              mut acquire: MessageWriter<TryAcquireObject>| {
            set_target.write(SetInteractionTarget {
                interactor,
                target: Some(click.entity),
            });
            acquire.write(TryAcquireObject { interactor });
        },
    );

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
