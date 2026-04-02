use std::{env, time::Duration};

use avian3d::prelude::{
    AngularDamping, Collider, CollisionLayers, LayerMask, LinearDamping, Mass, PhysicsPlugins,
    RigidBody, TransformInterpolation,
};
use bevy::{app::AppExit, prelude::*};
use bevy_enhanced_input::prelude::{Cancel as InputCancel, *};
use saddle_physics_object_interaction::{
    AdjustHoldDistance, CycleDirection, CycleInteractionTarget, HeldBy, HoldDistance,
    HoldOrientationMode, HoldOrientationOverride, HoldPointOverride, InteractableBody,
    InteractionCollisionPolicy, InteractionMassLimitOverride, InteractionTarget, ObjectAcquired,
    ObjectInteractionConfig, ObjectInteractionDebugSettings, ObjectInteractionDiagnostics,
    ObjectInteractionFailed, ObjectInteractionPlugin, ObjectInteractionState,
    ObjectInteractionSystems, ObjectInteractor, ObjectReleased, ObjectThrown,
    PreferredHoldDistance, ReleaseHeldObject, RotateHeldObject, SetInteractionTarget,
    ThrowHeldObject, ThrowResponseOverride, TryAcquireObject,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoMode {
    Basic,
    GravityGun,
    InspectRotate,
    PickingIntegration,
    Lab,
}

impl DemoMode {
    fn title(self) -> &'static str {
        match self {
            DemoMode::Basic => "object_interaction/basic",
            DemoMode::GravityGun => "object_interaction/gravity_gun",
            DemoMode::InspectRotate => "object_interaction/inspect_rotate",
            DemoMode::PickingIntegration => "object_interaction/picking_integration",
            DemoMode::Lab => "object_interaction_lab",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            DemoMode::Basic => {
                "E acquire, R release, F throw. Z/X adjust distance. A/D rotate. Tab cycles candidates."
            }
            DemoMode::GravityGun => {
                "Tuned for stronger pull and bigger throws. The heavy spool is intentionally liftable here."
            }
            DemoMode::InspectRotate => {
                "Short hold distance, aligned orientation, and close-up rotation for inspection flow."
            }
            DemoMode::PickingIntegration => {
                "Click a prop to set an explicit target and acquire it. Keyboard controls still work after pickup."
            }
            DemoMode::Lab => {
                "1 crate, 2 heavy, 3 inspect, 4 occlusion. E acquire, R release, F throw, Z/X distance, A/D rotate."
            }
        }
    }

    fn config(self) -> ObjectInteractionConfig {
        match self {
            DemoMode::GravityGun => ObjectInteractionConfig {
                acquisition: saddle_physics_object_interaction::AcquisitionConfig {
                    max_distance: 8.5,
                    forgiving_radius: 1.35,
                    max_target_mass: 120.0,
                    ..default()
                },
                hold: saddle_physics_object_interaction::HoldConfig {
                    default_distance: 3.1,
                    max_distance: 6.5,
                    linear_stiffness: 220.0,
                    linear_damping: 36.0,
                    max_force: 5_200.0,
                    max_torque: 240.0,
                    break_distance: 5.2,
                    ..default()
                },
                throw: saddle_physics_object_interaction::ThrowConfig {
                    impulse: 28.0,
                    angular_impulse: 4.6,
                    upward_bias: 0.12,
                    ..default()
                },
                ..default()
            },
            DemoMode::InspectRotate => ObjectInteractionConfig {
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
            },
            _ => ObjectInteractionConfig {
                acquisition: saddle_physics_object_interaction::AcquisitionConfig {
                    max_distance: 6.8,
                    ..default()
                },
                ..default()
            },
        }
    }

    fn initial_station(self) -> DemoStation {
        match self {
            DemoMode::InspectRotate => DemoStation::Inspect,
            _ => DemoStation::Crate,
        }
    }

    fn enable_mesh_picking(self) -> bool {
        matches!(self, DemoMode::PickingIntegration)
    }

    fn interactor_max_mass(self) -> Option<f32> {
        match self {
            DemoMode::GravityGun => Some(120.0),
            _ => Some(45.0),
        }
    }

    fn interactor_orientation(self) -> HoldOrientationMode {
        HoldOrientationMode::UseConfig
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum DemoStation {
    Crate,
    Heavy,
    Inspect,
    Occlusion,
}

#[derive(Resource, Clone, Copy)]
struct DemoModeResource(DemoMode);

#[derive(Resource)]
struct AutoExitTimer(Timer);

#[derive(Resource, Clone, Copy, Reflect)]
#[reflect(Resource)]
pub struct DemoWorld {
    pub interactor: Entity,
    pub light_crate: Entity,
    pub heavy_spool: Entity,
    pub inspect_prism: Entity,
    pub occlusion_wall: Entity,
}

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct DemoDiagnostics {
    pub station: Option<DemoStation>,
    pub target_name: Option<String>,
    pub held_name: Option<String>,
    pub hold_distance: f32,
    pub last_acquired_name: Option<String>,
    pub last_released_name: Option<String>,
    pub last_released_reason: Option<String>,
    pub last_failure_reason: Option<String>,
    pub last_failure_target: Option<String>,
    pub last_thrown_name: Option<String>,
    pub last_throw_impulse: Vec3,
    pub unstable_count: u32,
    pub acquisition_count: u32,
    pub release_count: u32,
    pub throw_count: u32,
}

#[derive(Component)]
struct DemoInteractor;

#[derive(Component)]
struct DemoInputContext;

#[derive(Component)]
struct DemoOverlay;

#[derive(Component)]
struct DemoPropVisual {
    base_color: Color,
}

#[derive(Component)]
struct DemoProp;

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

#[derive(InputAction)]
#[action_output(bool)]
struct PrevCycleAction;

#[derive(InputAction)]
#[action_output(bool)]
struct CrateStationAction;

#[derive(InputAction)]
#[action_output(bool)]
struct HeavyStationAction;

#[derive(InputAction)]
#[action_output(bool)]
struct InspectStationAction;

#[derive(InputAction)]
#[action_output(bool)]
struct OcclusionStationAction;

pub fn configure_app(app: &mut App, mode: DemoMode) {
    app.insert_resource(ClearColor(Color::srgb(0.04, 0.045, 0.055)));
    app.insert_resource(mode.config());
    app.insert_resource(ObjectInteractionDebugSettings {
        enabled: true,
        draw_gizmos: matches!(mode, DemoMode::Lab),
    });
    app.insert_resource(DemoModeResource(mode));
    app.init_resource::<DemoDiagnostics>();
    app.register_type::<DemoDiagnostics>();
    app.register_type::<DemoWorld>();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: mode.title().into(),
            resolution: (1440, 900).into(),
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(PhysicsPlugins::default());
    app.add_plugins(EnhancedInputPlugin);
    app.add_input_context::<DemoInputContext>();
    if mode.enable_mesh_picking() {
        app.add_plugins(MeshPickingPlugin);
    }
    app.add_plugins(ObjectInteractionPlugin::default());
    if let Some(auto_exit) = auto_exit_timer_from_env() {
        app.insert_resource(auto_exit);
        app.add_systems(Update, exit_after_timeout);
    }
    app.add_observer(on_acquire);
    app.add_observer(on_release);
    app.add_observer(on_throw);
    app.add_observer(on_throw_cancel);
    app.add_observer(on_cycle);
    app.add_observer(on_prev_cycle);
    app.add_observer(on_near);
    app.add_observer(on_far);
    app.add_observer(on_rotate_left);
    app.add_observer(on_rotate_right);
    app.add_observer(on_crate_station);
    app.add_observer(on_heavy_station);
    app.add_observer(on_inspect_station);
    app.add_observer(on_occlusion_station);
    app.add_systems(Startup, setup_scene);
    app.add_systems(
        Update,
        (
            tint_props.after(ObjectInteractionSystems::Presentation),
            record_acquired.after(ObjectInteractionSystems::Presentation),
            record_released.after(ObjectInteractionSystems::Presentation),
            record_failed.after(ObjectInteractionSystems::Presentation),
            record_thrown.after(ObjectInteractionSystems::Presentation),
            record_unstable.after(ObjectInteractionSystems::Presentation),
            refresh_demo_diagnostics.after(ObjectInteractionSystems::Presentation),
            update_overlay.after(ObjectInteractionSystems::Presentation),
        ),
    );
}

fn auto_exit_timer_from_env() -> Option<AutoExitTimer> {
    let seconds = env::var("OBJECT_INTERACTION_EXIT_AFTER_SECONDS")
        .ok()?
        .parse::<f32>()
        .ok()?;
    if seconds <= 0.0 {
        return None;
    }

    Some(AutoExitTimer(Timer::new(
        Duration::from_secs_f32(seconds),
        TimerMode::Once,
    )))
}

fn exit_after_timeout(
    time: Res<Time>,
    auto_exit: Option<ResMut<AutoExitTimer>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut auto_exit) = auto_exit else {
        return;
    };

    if auto_exit.0.tick(time.delta()).just_finished() {
        exit.write(AppExit::Success);
    }
}

pub fn set_station(world: &mut World, station: DemoStation) {
    let interactor = world.resource::<DemoWorld>().interactor;
    let transform = station_transform(station);
    world
        .entity_mut(interactor)
        .insert((transform, GlobalTransform::from(transform)));
    world.resource_mut::<DemoDiagnostics>().station = Some(station);
}

pub fn send_try_acquire(world: &mut World) {
    let interactor = world.resource::<DemoWorld>().interactor;
    world.write_message(TryAcquireObject { interactor });
}

pub fn send_release(world: &mut World) {
    let interactor = world.resource::<DemoWorld>().interactor;
    world.write_message(ReleaseHeldObject { interactor });
}

pub fn send_throw(world: &mut World) {
    let interactor = world.resource::<DemoWorld>().interactor;
    world.write_message(ThrowHeldObject {
        interactor,
        impulse_scale: 1.0,
        angular_impulse_scale: 1.0,
    });
}

pub fn send_adjust_distance(world: &mut World, delta: f32) {
    let interactor = world.resource::<DemoWorld>().interactor;
    world.write_message(AdjustHoldDistance { interactor, delta });
}

pub fn send_rotate_y(world: &mut World, degrees: f32) {
    let interactor = world.resource::<DemoWorld>().interactor;
    world.write_message(RotateHeldObject {
        interactor,
        delta: Quat::from_rotation_y(degrees.to_radians()),
    });
}

fn setup_scene(
    mut commands: Commands,
    mode: Res<DemoModeResource>,
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Ambient Light"),
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

    let actor_transform = station_transform(mode.0.initial_station());
    let interactor = commands
        .spawn((
            Name::new("Interactor"),
            DemoInteractor,
            DemoInputContext,
            ObjectInteractor {
                max_target_mass: mode.0.interactor_max_mass(),
                orientation_mode: mode.0.interactor_orientation(),
                ..default()
            },
            HoldDistance(mode.0.config().hold.default_distance),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            Transform::from_translation(actor_transform.translation).looking_at(
                actor_transform.translation + actor_transform.forward() * 4.0,
                Vec3::Y,
            ),
            GlobalTransform::IDENTITY,
            Visibility::Visible,
            actions!(DemoInputContext[
                (
                    Action::<AcquireAction>::new(),
                    bindings![KeyCode::KeyE, GamepadButton::South],
                ),
                (
                    Action::<ReleaseAction>::new(),
                    bindings![KeyCode::KeyR, GamepadButton::East],
                ),
                (
                    Action::<ThrowAction>::new(),
                    bindings![KeyCode::KeyF, MouseButton::Left],
                ),
                (
                    Action::<NearAction>::new(),
                    bindings![KeyCode::KeyZ],
                ),
                (
                    Action::<FarAction>::new(),
                    bindings![KeyCode::KeyX],
                ),
                (
                    Action::<RotateLeftAction>::new(),
                    bindings![KeyCode::KeyA],
                ),
                (
                    Action::<RotateRightAction>::new(),
                    bindings![KeyCode::KeyD],
                ),
                (
                    Action::<CycleAction>::new(),
                    bindings![KeyCode::Tab],
                ),
                (
                    Action::<PrevCycleAction>::new(),
                    bindings![KeyCode::KeyQ],
                ),
                (
                    Action::<CrateStationAction>::new(),
                    bindings![KeyCode::Digit1],
                ),
                (
                    Action::<HeavyStationAction>::new(),
                    bindings![KeyCode::Digit2],
                ),
                (
                    Action::<InspectStationAction>::new(),
                    bindings![KeyCode::Digit3],
                ),
                (
                    Action::<OcclusionStationAction>::new(),
                    bindings![KeyCode::Digit4],
                ),
            ]),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Interactor Camera"),
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        })
        .id();

    let light_crate = spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        mode.0,
        PropSpec {
            name: "Light Crate",
            mesh: Mesh::from(Cuboid::new(0.9, 0.9, 0.9)),
            collider: Collider::cuboid(0.45, 0.45, 0.45),
            position: Vec3::new(0.0, 0.75, 0.0),
            base_color: Color::srgb(0.76, 0.52, 0.24),
            mass: 8.0,
            extras: (PreferredHoldDistance(2.4), ThrowResponseOverride::default()),
        },
    );
    let heavy_spool = spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        mode.0,
        PropSpec {
            name: "Heavy Spool",
            mesh: Mesh::from(Cylinder::new(0.52, 0.75)),
            collider: Collider::cylinder(0.52, 0.75),
            position: Vec3::new(-2.6, 0.82, -0.2),
            base_color: Color::srgb(0.34, 0.44, 0.78),
            mass: 80.0,
            extras: InteractionMassLimitOverride(80.0),
        },
    );
    let inspect_prism = spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        interactor,
        mode.0,
        PropSpec {
            name: "Inspect Prism",
            mesh: Mesh::from(Sphere::new(0.45).mesh().ico(5).expect("ico sphere")),
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

    let occlusion_wall = commands
        .spawn((
            Name::new("Occlusion Wall"),
            Mesh3d(meshes.add(Cuboid::new(0.35, 2.2, 1.8))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.36, 0.38, 0.44),
                perceptual_roughness: 0.92,
                ..default()
            })),
            RigidBody::Static,
            Collider::cuboid(0.175, 1.1, 0.9),
            Transform::from_xyz(0.86, 1.0, 3.9),
            Pickable::IGNORE,
        ))
        .id();

    commands.spawn((
        Name::new("Demo Overlay"),
        DemoOverlay,
        Text::new(String::new()),
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

    commands.insert_resource(DemoWorld {
        interactor,
        light_crate,
        heavy_spool,
        inspect_prism,
        occlusion_wall,
    });
    diagnostics.station = Some(mode.0.initial_station());
}

struct PropSpec<B: Bundle> {
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
    interactor: Entity,
    mode: DemoMode,
    spec: PropSpec<B>,
) -> Entity {
    let mut entity = commands.spawn((
        Name::new(spec.name),
        DemoProp,
        DemoPropVisual {
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
        TransformInterpolation::default(),
        CollisionLayers::new(0b0001, LayerMask::ALL),
        Transform::from_translation(spec.position),
        GlobalTransform::IDENTITY,
    ));
    entity.insert(spec.extras);

    if mode.enable_mesh_picking() {
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
    }

    entity.id()
}

fn station_transform(station: DemoStation) -> Transform {
    let (eye, target) = match station {
        DemoStation::Crate => (Vec3::new(0.0, 1.45, 5.4), Vec3::new(0.0, 0.9, 0.0)),
        DemoStation::Heavy => (Vec3::new(0.0, 1.45, 5.4), Vec3::new(-2.6, 0.95, -0.2)),
        DemoStation::Inspect => (Vec3::new(1.75, 1.45, 4.55), Vec3::new(2.4, 0.9, -0.05)),
        DemoStation::Occlusion => (Vec3::new(1.55, 1.45, 5.35), Vec3::new(0.0, 0.9, 0.0)),
    };
    Transform::from_translation(eye).looking_at(target, Vec3::Y)
}

fn on_acquire(trigger: On<Start<AcquireAction>>, mut writer: MessageWriter<TryAcquireObject>) {
    writer.write(TryAcquireObject {
        interactor: trigger.context,
    });
}

fn on_release(trigger: On<Start<ReleaseAction>>, mut writer: MessageWriter<ReleaseHeldObject>) {
    writer.write(ReleaseHeldObject {
        interactor: trigger.context,
    });
}

fn on_throw(trigger: On<Start<ThrowAction>>, mut writer: MessageWriter<ThrowHeldObject>) {
    writer.write(ThrowHeldObject {
        interactor: trigger.context,
        impulse_scale: 1.0,
        angular_impulse_scale: 1.0,
    });
}

fn on_throw_cancel(
    trigger: On<InputCancel<ThrowAction>>,
    mut writer: MessageWriter<ReleaseHeldObject>,
) {
    writer.write(ReleaseHeldObject {
        interactor: trigger.context,
    });
}

fn on_cycle(trigger: On<Start<CycleAction>>, mut writer: MessageWriter<CycleInteractionTarget>) {
    writer.write(CycleInteractionTarget {
        interactor: trigger.context,
        direction: CycleDirection::Next,
    });
}

fn on_prev_cycle(
    trigger: On<Start<PrevCycleAction>>,
    mut writer: MessageWriter<CycleInteractionTarget>,
) {
    writer.write(CycleInteractionTarget {
        interactor: trigger.context,
        direction: CycleDirection::Previous,
    });
}

fn on_near(trigger: On<Start<NearAction>>, mut writer: MessageWriter<AdjustHoldDistance>) {
    writer.write(AdjustHoldDistance {
        interactor: trigger.context,
        delta: -0.25,
    });
}

fn on_far(trigger: On<Start<FarAction>>, mut writer: MessageWriter<AdjustHoldDistance>) {
    writer.write(AdjustHoldDistance {
        interactor: trigger.context,
        delta: 0.25,
    });
}

fn on_rotate_left(
    trigger: On<Start<RotateLeftAction>>,
    mut writer: MessageWriter<RotateHeldObject>,
) {
    writer.write(RotateHeldObject {
        interactor: trigger.context,
        delta: Quat::from_rotation_y(18.0_f32.to_radians()),
    });
}

fn on_rotate_right(
    trigger: On<Start<RotateRightAction>>,
    mut writer: MessageWriter<RotateHeldObject>,
) {
    writer.write(RotateHeldObject {
        interactor: trigger.context,
        delta: Quat::from_rotation_y((-18.0_f32).to_radians()),
    });
}

fn on_crate_station(_trigger: On<Start<CrateStationAction>>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        set_station(world, DemoStation::Crate);
    });
}

fn on_heavy_station(_trigger: On<Start<HeavyStationAction>>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        set_station(world, DemoStation::Heavy);
    });
}

fn on_inspect_station(_trigger: On<Start<InspectStationAction>>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        set_station(world, DemoStation::Inspect);
    });
}

fn on_occlusion_station(_trigger: On<Start<OcclusionStationAction>>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        set_station(world, DemoStation::Occlusion);
    });
}

fn tint_props(
    demo: Res<DemoWorld>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_actor: Query<(&InteractionTarget, &ObjectInteractionState)>,
    q_props: Query<
        (
            Entity,
            &DemoPropVisual,
            &MeshMaterial3d<StandardMaterial>,
            Option<&HeldBy>,
        ),
        With<DemoProp>,
    >,
) {
    let Ok((target, state)) = q_actor.get(demo.interactor) else {
        return;
    };
    let targeted = target.entity;
    let held = match *state {
        ObjectInteractionState::Holding(entity) => Some(entity),
        _ => None,
    };

    for (entity, visual, material, held_by) in &q_props {
        let color = if held == Some(entity)
            || held_by.is_some_and(|held_by| held_by.0 == demo.interactor)
        {
            Color::srgb(0.28, 0.96, 0.58)
        } else if targeted == Some(entity) {
            Color::srgb(0.98, 0.80, 0.24)
        } else {
            visual.base_color
        };

        if let Some(material) = materials.get_mut(&material.0) {
            material.base_color = color;
        }
    }
}

fn record_acquired(
    names: Query<&Name>,
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut reader: MessageReader<ObjectAcquired>,
) {
    for event in reader.read() {
        diagnostics.acquisition_count += 1;
        diagnostics.last_acquired_name = names.get(event.object).ok().map(ToString::to_string);
    }
}

fn record_released(
    names: Query<&Name>,
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut reader: MessageReader<ObjectReleased>,
) {
    for event in reader.read() {
        diagnostics.release_count += 1;
        diagnostics.last_released_name = names.get(event.object).ok().map(ToString::to_string);
        diagnostics.last_released_reason = Some(format!("{:?}", event.reason));
    }
}

fn record_failed(
    names: Query<&Name>,
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut reader: MessageReader<ObjectInteractionFailed>,
) {
    for event in reader.read() {
        diagnostics.last_failure_reason = Some(format!("{:?}", event.reason));
        diagnostics.last_failure_target = event
            .target
            .and_then(|entity| names.get(entity).ok())
            .map(ToString::to_string);
    }
}

fn record_thrown(
    names: Query<&Name>,
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut reader: MessageReader<ObjectThrown>,
) {
    for event in reader.read() {
        diagnostics.throw_count += 1;
        diagnostics.last_thrown_name = names.get(event.object).ok().map(ToString::to_string);
        diagnostics.last_throw_impulse = event.impulse;
    }
}

fn record_unstable(
    mut diagnostics: ResMut<DemoDiagnostics>,
    mut reader: MessageReader<saddle_physics_object_interaction::HeldObjectBecameUnstable>,
) {
    for _ in reader.read() {
        diagnostics.unstable_count += 1;
    }
}

fn refresh_demo_diagnostics(
    demo: Res<DemoWorld>,
    names: Query<&Name>,
    q_actor: Query<(&HoldDistance, &InteractionTarget, &ObjectInteractionState)>,
    object_diagnostics: Res<ObjectInteractionDiagnostics>,
    mut diagnostics: ResMut<DemoDiagnostics>,
) {
    let Ok((hold_distance, target, state)) = q_actor.get(demo.interactor) else {
        return;
    };

    diagnostics.hold_distance = hold_distance.0;
    diagnostics.target_name = target
        .entity
        .and_then(|entity| names.get(entity).ok())
        .map(ToString::to_string);
    diagnostics.held_name = match *state {
        ObjectInteractionState::Holding(entity) => names.get(entity).ok().map(ToString::to_string),
        _ => None,
    };

    if let Some(last_failure) = &object_diagnostics.last_failure {
        diagnostics.last_failure_reason = Some(format!("{:?}", last_failure.reason));
    }
    if let Some(last_release) = &object_diagnostics.last_release {
        diagnostics.last_released_reason = Some(format!("{:?}", last_release.reason));
    }
}

fn update_overlay(
    mode: Res<DemoModeResource>,
    diagnostics: Res<DemoDiagnostics>,
    mut overlay: Single<&mut Text, With<DemoOverlay>>,
) {
    **overlay = Text::new(format!(
        "{}\n{}\nstation: {}\ntarget: {}\nheld: {}\nhold distance: {:.2}\nacquired: {} released: {} thrown: {}\nlast release: {}\nlast failure: {}\nlast throw impulse: {:.2?}",
        mode.0.title(),
        mode.0.subtitle(),
        diagnostics
            .station
            .map(|station| format!("{station:?}"))
            .unwrap_or_else(|| "unknown".to_owned()),
        diagnostics.target_name.as_deref().unwrap_or("none"),
        diagnostics.held_name.as_deref().unwrap_or("none"),
        diagnostics.hold_distance,
        diagnostics.acquisition_count,
        diagnostics.release_count,
        diagnostics.throw_count,
        diagnostics
            .last_released_reason
            .as_deref()
            .unwrap_or("none"),
        diagnostics.last_failure_reason.as_deref().unwrap_or("none"),
        diagnostics.last_throw_impulse,
    ));
}
