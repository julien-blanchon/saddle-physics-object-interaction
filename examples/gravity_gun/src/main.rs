//! Gravity-gun-style interaction with stronger pull, heavier mass limit, and powerful throws.
//!
//! Press **E** to acquire, **R** to release, **F** to throw with force.
//! **Z/X** adjust hold distance, **A/D** rotate, **Tab** cycles candidates.

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
    InteractableBody, InteractionMassLimitOverride, InteractionTarget,
    ObjectInteractionConfig, ObjectInteractionDebugSettings, ObjectInteractionPlugin,
    ObjectInteractionState, ObjectInteractionSystems, ObjectInteractor, PreferredHoldDistance,
    ReleaseHeldObject, RotateHeldObject, ThrowHeldObject, ThrowResponseOverride, TryAcquireObject,
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
    // Gravity-gun tuning: longer reach, heavier mass limit, stronger spring, bigger throws
    let config = ObjectInteractionConfig {
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
            title: "object_interaction / gravity_gun".into(),
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

    // -- Interactor with 120 kg mass limit (gravity-gun strength) -----------
    commands
        .spawn((
            Name::new("Interactor"),
            Interactor,
            InputCtx,
            ObjectInteractor {
                max_target_mass: Some(120.0),
                ..default()
            },
            HoldDistance(3.1),
            CollisionLayers::new(0b0010, LayerMask::ALL),
            Transform::from_xyz(0.0, 1.45, 5.4).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
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

    // -- Props ---------------------------------------------------------------

    // Light crate -- easily grabbed and thrown far
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
            extras: (PreferredHoldDistance(2.4), ThrowResponseOverride::default()),
        },
    );

    // Heavy spool -- liftable thanks to the 120 kg limit and mass limit override
    spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        PropDef {
            name: "Heavy Spool",
            mesh: Mesh::from(Cylinder::new(0.52, 0.75)),
            collider: Collider::cylinder(0.52, 0.75),
            position: Vec3::new(-2.6, 0.82, -0.2),
            base_color: Color::srgb(0.34, 0.44, 0.78),
            mass: 80.0,
            extras: InteractionMassLimitOverride(80.0),
        },
    );

    // Small sphere -- lightweight, launches impressively with 28 impulse
    spawn_prop(
        &mut commands,
        &mut meshes,
        &mut materials,
        PropDef {
            name: "Small Sphere",
            mesh: Sphere::new(0.35).mesh().ico(4).expect("ico sphere"),
            collider: Collider::sphere(0.35),
            position: Vec3::new(2.0, 0.65, 0.5),
            base_color: Color::srgb(0.82, 0.82, 0.90),
            mass: 3.0,
            extras: (),
        },
    );

    // HUD
    commands.spawn((
        Name::new("HUD"),
        Text::new(
            "object_interaction / gravity_gun\n\
             Tuned for stronger pull and bigger throws. The heavy spool is intentionally liftable.\n\
             E acquire | R release | F throw | Z/X distance | A/D rotate | Tab cycle",
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
