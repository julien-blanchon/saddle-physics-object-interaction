use avian3d::prelude::{CollisionLayers, SpatialQueryFilter};
use bevy::prelude::*;

use crate::messages::{AcquireFailureReason, ReleaseReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[reflect(Debug, PartialEq, Default)]
pub enum AcquisitionMode {
    RaycastOnly,
    OverlapOnly,
    #[default]
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[reflect(Debug, PartialEq, Default)]
pub enum CandidateMethod {
    #[default]
    Overlap,
    DirectHit,
    ExplicitTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[reflect(Debug, PartialEq, Default)]
pub enum HoldAnchorMode {
    #[default]
    CenterOfMass,
    HitPoint,
    CustomLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[reflect(Debug, PartialEq, Default)]
pub enum HoldOrientationMode {
    #[default]
    UseConfig,
    PreserveWorld,
    AlignToInteractor,
    CustomLocal,
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct InteractionAnchor {
    pub local_offset: Vec3,
}

impl Default for InteractionAnchor {
    fn default() -> Self {
        Self {
            local_offset: Vec3::new(0.0, 0.0, -0.65),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct HoldDistance(pub f32);

impl Default for HoldDistance {
    fn default() -> Self {
        Self(2.5)
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct InteractionTarget {
    pub entity: Option<Entity>,
    pub score: f32,
    pub method: CandidateMethod,
    pub hit_point: Option<Vec3>,
}

impl Default for InteractionTarget {
    fn default() -> Self {
        Self {
            entity: None,
            score: 0.0,
            method: CandidateMethod::Overlap,
            hit_point: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, Component, Default)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct InteractionCandidates {
    pub ordered: Vec<Entity>,
    pub selected: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Component, Default)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct SurfacePlacementMode {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Component, Default)]
#[reflect(Debug, Component, PartialEq, Default)]
pub enum ObjectInteractionState {
    #[default]
    Idle,
    Targeting {
        entity: Entity,
        method: CandidateMethod,
    },
    Holding(Entity),
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
#[require(
    ObjectInteractionState,
    InteractionAnchor,
    HoldDistance,
    InteractionTarget,
    InteractionCandidates,
    SurfacePlacementMode,
    ObjectInteractionCommandState
)]
pub struct ObjectInteractor {
    pub enabled: bool,
    pub candidate_filter: SpatialQueryFilter,
    pub obstruction_filter: SpatialQueryFilter,
    pub acquisition_mode: AcquisitionMode,
    pub max_target_mass: Option<f32>,
    pub orientation_mode: HoldOrientationMode,
}

impl Default for ObjectInteractor {
    fn default() -> Self {
        Self {
            enabled: true,
            candidate_filter: SpatialQueryFilter::default(),
            obstruction_filter: SpatialQueryFilter::default(),
            acquisition_mode: AcquisitionMode::Hybrid,
            max_target_mass: None,
            orientation_mode: HoldOrientationMode::UseConfig,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct InteractableBody {
    pub enabled: bool,
    pub priority: f32,
    pub anchor_mode: HoldAnchorMode,
}

impl Default for InteractableBody {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 0.0,
            anchor_mode: HoldAnchorMode::CenterOfMass,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct PreferredHoldDistance(pub f32);

impl Default for PreferredHoldDistance {
    fn default() -> Self {
        Self(2.5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq, Default)]
pub struct InteractionMassLimitOverride(pub f32);

impl Default for InteractionMassLimitOverride {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct HoldPointOverride {
    pub local_offset: Vec3,
    pub local_rotation: Quat,
}

impl Default for HoldPointOverride {
    fn default() -> Self {
        Self {
            local_offset: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct HoldOrientationOverride {
    pub mode: HoldOrientationMode,
}

impl Default for HoldOrientationOverride {
    fn default() -> Self {
        Self {
            mode: HoldOrientationMode::UseConfig,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct ThrowResponseOverride {
    pub impulse_scale: f32,
    pub angular_impulse_scale: f32,
    pub inherit_actor_velocity: Option<bool>,
    pub upward_bias_scale: f32,
}

impl Default for ThrowResponseOverride {
    fn default() -> Self {
        Self {
            impulse_scale: 1.0,
            angular_impulse_scale: 1.0,
            inherit_actor_velocity: None,
            upward_bias_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Component, Default)]
#[reflect(Debug, Component, PartialEq, Default)]
pub enum InteractionCollisionPolicy {
    #[default]
    Preserve,
    IgnoreInteractorLayer,
    DisableAll,
    CustomLayers(CollisionLayers),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct Holding(pub Entity);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Component)]
#[reflect(Debug, Component, PartialEq)]
pub struct HeldBy(pub Entity);

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingThrow {
    pub impulse_scale: f32,
    pub angular_impulse_scale: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SavedCollisionLayers {
    Absent,
    Present(CollisionLayers),
}

#[derive(Debug, Clone, Component)]
pub(crate) struct ObjectInteractionCommandState {
    pub acquire_requested: bool,
    pub pending_release: Option<ReleaseReason>,
    pub pending_throw: Option<PendingThrow>,
    pub cycle_steps: i32,
    pub override_target: Option<Entity>,
    pub last_rejected_target: Option<(Entity, AcquireFailureReason)>,
    pub rotation_delta: Quat,
}

impl Default for ObjectInteractionCommandState {
    fn default() -> Self {
        Self {
            acquire_requested: false,
            pending_release: None,
            pending_throw: None,
            cycle_steps: 0,
            override_target: None,
            last_rejected_target: None,
            rotation_delta: Quat::IDENTITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub(crate) struct HeldRuntime {
    pub local_anchor: Vec3,
    pub base_rotation_offset: Quat,
    pub rotation_adjustment: Quat,
    pub last_target_position: Vec3,
    pub last_target_rotation: Quat,
    pub pull_elapsed: f32,
    pub pull_duration: f32,
    pub pull_start_distance: f32,
    pub pull_target_distance: f32,
    pub unstable_seconds: f32,
    pub occluded_seconds: f32,
    pub last_force: Vec3,
    pub last_torque: Vec3,
    pub saved_collision_layers: Option<SavedCollisionLayers>,
}

impl HeldRuntime {
    pub(crate) fn new(
        local_anchor: Vec3,
        base_rotation_offset: Quat,
        last_target_position: Vec3,
        last_target_rotation: Quat,
        saved_collision_layers: Option<SavedCollisionLayers>,
    ) -> Self {
        Self {
            local_anchor,
            base_rotation_offset,
            rotation_adjustment: Quat::IDENTITY,
            last_target_position,
            last_target_rotation,
            pull_elapsed: 0.0,
            pull_duration: 0.0,
            pull_start_distance: 0.0,
            pull_target_distance: 0.0,
            unstable_seconds: 0.0,
            occluded_seconds: 0.0,
            last_force: Vec3::ZERO,
            last_torque: Vec3::ZERO,
            saved_collision_layers,
        }
    }
}
