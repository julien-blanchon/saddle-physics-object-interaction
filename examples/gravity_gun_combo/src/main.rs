use avian3d::prelude::{AngularVelocity, Collider, LinearVelocity, RigidBody};
use bevy::prelude::*;
use saddle_pane::prelude::*;
use saddle_physics_destruction::{
    ApplyDestructionDamage, ChunkGroupDetached, CuboidAnchorPreset, CuboidFractureBuilder,
    Destructible, DestructionAssetHandle, DestructionConfig, DestructionDiagnostics,
    DestructionPlugin, DestructionState, FracturedAsset, Fragment, FragmentSpawnData, MaterialHint,
    RootVisualMode, build_fragment_mesh,
};
use saddle_physics_object_interaction::{HeldBy, ObjectThrown};
use saddle_physics_object_interaction_example_common as common;
use saddle_physics_transform_interpolation::{
    TransformInterpolation as SmoothTransformInterpolation, TransformInterpolationAxes,
    TransformInterpolationConfig, TransformInterpolationPlugin,
};

#[derive(Component)]
struct ComboBarrier;

#[derive(Component)]
struct ExplosiveBarrel {
    home: Vec3,
    armed: bool,
}

#[derive(Component)]
struct ComboFragmentMotion {
    linear_velocity: Vec3,
    angular_velocity: Vec3,
}

#[derive(Resource)]
struct ComboWorld {
    barrier: Entity,
}

#[derive(Resource, Pane)]
#[pane(title = "Combo Demo", position = "top-left")]
struct ComboPane {
    #[pane(slider, min = 0.4, max = 2.5, step = 0.05)]
    detonation_distance: f32,
    #[pane(slider, min = 2.0, max = 24.0, step = 0.25)]
    damage_magnitude: f32,
    #[pane(slider, min = 0.3, max = 4.0, step = 0.05)]
    damage_radius: f32,
    #[pane(slider, min = 1.0, max = 18.0, step = 0.25)]
    fragment_gravity: f32,
    #[pane(slider, min = 0.5, max = 10.0, step = 0.1)]
    fragment_lifetime_secs: f32,
    #[pane(slider, min = 0.1, max = 12.0, step = 0.1)]
    translation_snap_distance: f32,
}

impl ComboPane {
    fn demo_defaults() -> Self {
        Self {
            detonation_distance: 1.2,
            damage_magnitude: 8.0,
            damage_radius: 1.6,
            fragment_gravity: 7.5,
            fragment_lifetime_secs: 6.0,
            translation_snap_distance: 4.0,
        }
    }
}

impl FromWorld for ComboPane {
    fn from_world(world: &mut World) -> Self {
        let destruction = world.resource::<DestructionConfig>().clone();
        let interpolation = world.resource::<TransformInterpolationConfig>().clone();
        Self {
            fragment_lifetime_secs: destruction.default_fragment_lifetime_secs,
            translation_snap_distance: interpolation.translation_snap_distance,
            ..Self::demo_defaults()
        }
    }
}

#[derive(Resource, Default, Pane)]
#[pane(title = "Combo Stats", position = "bottom-left")]
struct ComboStatsPane {
    #[pane(monitor)]
    armed_barrel: bool,
    #[pane(monitor)]
    barrier_broken: bool,
    #[pane(monitor)]
    detonations: u32,
    #[pane(monitor)]
    active_fragments: usize,
}

#[derive(Resource, Default)]
struct ComboRuntime {
    detonations: u32,
    armed_barrel: bool,
}

fn main() {
    let mut app = App::new();
    common::configure_app(&mut app, common::DemoMode::GravityGun);
    app.insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(DestructionConfig {
            default_fragment_lifetime_secs: 6.0,
            fragment_fade_secs: 1.1,
            max_fragment_distance: 40.0,
            max_chunk_spawns_per_frame: 32,
            ..default()
        })
        .init_resource::<ComboRuntime>()
        .add_plugins((
            DestructionPlugin::default(),
            TransformInterpolationPlugin::default(),
        ))
        .register_pane::<ComboPane>()
        .register_pane::<ComboStatsPane>()
        .add_systems(PostStartup, setup_combo_room)
        .add_systems(
            Update,
            (
                sync_combo_pane,
                arm_explosive_barrels,
                detonate_explosive_barrels,
                clear_barrier_collider_on_break,
                materialize_fragments,
                sync_combo_stats,
            ),
        )
        .add_systems(FixedUpdate, advance_fragment_motion);
    app.run();
}

fn setup_combo_room(
    mut commands: Commands,
    demo_world: Res<common::DemoWorld>,
    mut fracture_assets: ResMut<Assets<FracturedAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands
        .entity(demo_world.light_crate)
        .insert(ExplosiveBarrel {
            home: Vec3::new(0.0, 0.75, 0.0),
            armed: false,
        });

    commands
        .entity(demo_world.light_crate)
        .with_children(|parent| {
            parent.spawn((
                Name::new("Explosive Barrel Beacon"),
                PointLight {
                    color: Color::srgb(1.0, 0.52, 0.28),
                    intensity: 48_000.0,
                    range: 5.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.9, 0.0),
            ));
        });

    let mut barrier_builder =
        CuboidFractureBuilder::new(Vec3::new(3.4, 2.6, 0.45), UVec3::new(4, 3, 1));
    barrier_builder.anchor_preset = CuboidAnchorPreset::Bottom;
    barrier_builder.material_hint = MaterialHint::Concrete;
    barrier_builder.seed = 64;
    let barrier_asset = barrier_builder.build();
    let barrier_handle = fracture_assets.add(barrier_asset);

    let barrier = commands
        .spawn((
            Name::new("Containment Barrier"),
            ComboBarrier,
            Destructible {
                visual_mode: RootVisualMode::HideOnFirstDetach,
            },
            DestructionAssetHandle(barrier_handle),
            Mesh3d(meshes.add(Cuboid::new(3.4, 2.6, 0.45))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.56, 0.62, 0.68),
                emissive: LinearRgba::rgb(0.02, 0.04, 0.06),
                perceptual_roughness: 0.9,
                ..default()
            })),
            RigidBody::Static,
            Collider::cuboid(3.4, 2.6, 0.45),
            Transform::from_xyz(0.0, 1.3, -2.25),
        ))
        .id();

    for x in [-2.15, 2.15] {
        commands.spawn((
            Name::new(format!("Barrier Pylon {x:.1}")),
            Mesh3d(meshes.add(Cuboid::new(0.38, 3.2, 0.38))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.18, 0.2, 0.26),
                metallic: 0.2,
                perceptual_roughness: 0.48,
                ..default()
            })),
            Transform::from_xyz(x, 1.6, -2.2),
        ));
        commands.spawn((
            Name::new(format!("Barrier Light {x:.1}")),
            PointLight {
                color: Color::srgb(0.42, 0.82, 1.0),
                intensity: 190_000.0,
                range: 8.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x * 0.92, 2.35, -1.8),
        ));
    }

    commands.spawn((
        Name::new("Barrier Sign"),
        Text::new("Launch the orange crate through the shield"),
        Node {
            position_type: PositionType::Absolute,
            right: px(22.0),
            top: px(18.0),
            width: px(360.0),
            ..default()
        },
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 0.98)),
    ));

    commands.insert_resource(ComboWorld { barrier });
}

fn sync_combo_pane(
    pane: Res<ComboPane>,
    mut destruction: ResMut<DestructionConfig>,
    mut interpolation: ResMut<TransformInterpolationConfig>,
) {
    if !pane.is_changed() && !pane.is_added() {
        return;
    }

    destruction.default_fragment_lifetime_secs = pane.fragment_lifetime_secs;
    interpolation.translation_snap_distance = pane.translation_snap_distance;
}

fn arm_explosive_barrels(
    mut runtime: ResMut<ComboRuntime>,
    mut reader: MessageReader<ObjectThrown>,
    mut explosives: Query<&mut ExplosiveBarrel>,
) {
    for message in reader.read() {
        let Ok(mut explosive) = explosives.get_mut(message.object) else {
            continue;
        };
        explosive.armed = true;
        runtime.armed_barrel = true;
    }
}

fn detonate_explosive_barrels(
    combo_world: Res<ComboWorld>,
    pane: Res<ComboPane>,
    mut runtime: ResMut<ComboRuntime>,
    barrier_state: Query<&DestructionState, With<ComboBarrier>>,
    barrier_transform: Query<&GlobalTransform, With<ComboBarrier>>,
    mut writer: MessageWriter<ApplyDestructionDamage>,
    mut explosives: Query<
        (
            &GlobalTransform,
            &mut Transform,
            &mut ExplosiveBarrel,
            Option<&mut LinearVelocity>,
            Option<&mut AngularVelocity>,
        ),
        Without<HeldBy>,
    >,
) {
    let Ok(state) = barrier_state.get(combo_world.barrier) else {
        return;
    };
    if state.broken || state.detached_chunks > 0 {
        runtime.armed_barrel = false;
        return;
    }

    let Ok(barrier_transform) = barrier_transform.get(combo_world.barrier) else {
        return;
    };
    let barrier_position = barrier_transform.translation();

    for (global_transform, mut transform, mut explosive, linear_velocity, angular_velocity) in
        &mut explosives
    {
        if !explosive.armed {
            continue;
        }

        if global_transform.translation().distance(barrier_position) > pane.detonation_distance {
            continue;
        }

        writer.write(ApplyDestructionDamage::radial(
            combo_world.barrier,
            global_transform.translation(),
            pane.damage_magnitude,
            pane.damage_radius,
        ));

        explosive.armed = false;
        runtime.armed_barrel = false;
        runtime.detonations += 1;
        transform.translation = explosive.home;
        transform.rotation = Quat::IDENTITY;
        if let Some(mut linear_velocity) = linear_velocity {
            *linear_velocity = LinearVelocity::ZERO;
        }
        if let Some(mut angular_velocity) = angular_velocity {
            *angular_velocity = AngularVelocity::ZERO;
        }
    }
}

fn clear_barrier_collider_on_break(
    combo_world: Res<ComboWorld>,
    mut commands: Commands,
    mut reader: MessageReader<ChunkGroupDetached>,
) {
    for message in reader.read() {
        if message.source != combo_world.barrier {
            continue;
        }

        commands.entity(combo_world.barrier).remove::<RigidBody>();
        commands.entity(combo_world.barrier).remove::<Collider>();
        break;
    }
}

fn materialize_fragments(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fragments: Query<(Entity, &Fragment, &FragmentSpawnData), Added<Fragment>>,
) {
    for (entity, fragment, spawn_data) in &fragments {
        commands.entity(entity).insert((
            Name::new(format!("Combo Fragment {}", fragment.primary_chunk.0)),
            Mesh3d(meshes.add(build_fragment_mesh(&spawn_data.render))),
            MeshMaterial3d(materials.add(material_for_hint(fragment.material_hint, 1.0))),
            ComboFragmentMotion {
                linear_velocity: spawn_data.initial_velocity.linear,
                angular_velocity: spawn_data.initial_velocity.angular,
            },
            SmoothTransformInterpolation::default()
                .with_axes(TransformInterpolationAxes::TRANSLATION_AND_ROTATION),
        ));
    }
}

fn advance_fragment_motion(
    time: Res<Time<Fixed>>,
    pane: Res<ComboPane>,
    mut fragments: Query<(&mut Transform, &mut ComboFragmentMotion)>,
) {
    let delta = time.delta_secs();
    for (mut transform, mut motion) in &mut fragments {
        motion.linear_velocity += Vec3::new(0.0, -pane.fragment_gravity, 0.0) * delta;
        transform.translation += motion.linear_velocity * delta;
        transform.rotate(Quat::from_euler(
            EulerRot::XYZ,
            motion.angular_velocity.x * delta,
            motion.angular_velocity.y * delta,
            motion.angular_velocity.z * delta,
        ));
    }
}

fn sync_combo_stats(
    runtime: Res<ComboRuntime>,
    destruction: Res<DestructionDiagnostics>,
    barrier_state: Query<&DestructionState, With<ComboBarrier>>,
    mut pane: ResMut<ComboStatsPane>,
) {
    pane.armed_barrel = runtime.armed_barrel;
    pane.detonations = runtime.detonations;
    pane.active_fragments = destruction.active_fragments;
    pane.barrier_broken = barrier_state
        .iter()
        .next()
        .is_some_and(|state| state.broken || state.detached_chunks > 0);
}

fn material_for_hint(material_hint: MaterialHint, alpha: f32) -> StandardMaterial {
    match material_hint {
        MaterialHint::Glass => StandardMaterial {
            base_color: Color::srgba(0.66, 0.82, 0.94, alpha * 0.48),
            alpha_mode: AlphaMode::Blend,
            emissive: LinearRgba::rgb(0.04, 0.06, 0.08),
            perceptual_roughness: 0.06,
            reflectance: 0.72,
            ..default()
        },
        MaterialHint::Wood => StandardMaterial {
            base_color: Color::srgba(0.58, 0.39, 0.24, alpha),
            perceptual_roughness: 0.78,
            ..default()
        },
        MaterialHint::Stone | MaterialHint::Concrete => StandardMaterial {
            base_color: Color::srgba(0.68, 0.7, 0.74, alpha),
            perceptual_roughness: 0.92,
            ..default()
        },
        MaterialHint::Metal => StandardMaterial {
            base_color: Color::srgba(0.7, 0.74, 0.79, alpha),
            metallic: 0.82,
            perceptual_roughness: 0.24,
            ..default()
        },
        MaterialHint::Ceramic => StandardMaterial {
            base_color: Color::srgba(0.84, 0.82, 0.78, alpha),
            perceptual_roughness: 0.35,
            ..default()
        },
        MaterialHint::Generic => StandardMaterial {
            base_color: Color::srgba(0.78, 0.56, 0.34, alpha),
            perceptual_roughness: 0.68,
            ..default()
        },
    }
}
