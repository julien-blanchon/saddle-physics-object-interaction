use bevy::prelude::*;

use crate::components::{HoldOrientationMode, InteractionCollisionPolicy};

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct AcquisitionConfig {
    pub max_distance: f32,
    pub forgiving_radius: f32,
    pub cone_half_angle_degrees: f32,
    pub max_target_mass: f32,
    pub require_line_of_sight: bool,
    pub sticky_target_bonus: f32,
    pub target_switch_hysteresis: f32,
}

impl Default for AcquisitionConfig {
    fn default() -> Self {
        Self {
            max_distance: 6.5,
            forgiving_radius: 1.1,
            cone_half_angle_degrees: 20.0,
            max_target_mass: 45.0,
            require_line_of_sight: true,
            sticky_target_bonus: 0.12,
            target_switch_hysteresis: 0.08,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct TargetScoringConfig {
    pub distance_weight: f32,
    pub angle_weight: f32,
    pub priority_weight: f32,
    pub direct_hit_bonus: f32,
}

impl Default for TargetScoringConfig {
    fn default() -> Self {
        Self {
            distance_weight: 0.35,
            angle_weight: 0.45,
            priority_weight: 0.2,
            direct_hit_bonus: 0.18,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct HoldConfig {
    pub min_distance: f32,
    pub default_distance: f32,
    pub max_distance: f32,
    pub linear_stiffness: f32,
    pub linear_damping: f32,
    pub angular_stiffness: f32,
    pub angular_damping: f32,
    pub max_force: f32,
    pub max_torque: f32,
    pub break_distance: f32,
    pub instability_distance: f32,
    pub instability_grace_seconds: f32,
    pub occlusion_grace_seconds: f32,
    pub collision_policy: InteractionCollisionPolicy,
    pub orientation_mode: HoldOrientationMode,
}

impl Default for HoldConfig {
    fn default() -> Self {
        Self {
            min_distance: 0.75,
            default_distance: 2.5,
            max_distance: 5.5,
            linear_stiffness: 150.0,
            linear_damping: 28.0,
            angular_stiffness: 64.0,
            angular_damping: 12.0,
            max_force: 2_800.0,
            max_torque: 180.0,
            break_distance: 4.2,
            instability_distance: 1.1,
            instability_grace_seconds: 0.35,
            occlusion_grace_seconds: 0.28,
            collision_policy: InteractionCollisionPolicy::IgnoreInteractorLayer,
            orientation_mode: HoldOrientationMode::PreserveWorld,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ThrowConfig {
    pub impulse: f32,
    pub angular_impulse: f32,
    pub upward_bias: f32,
    pub inherit_actor_velocity: bool,
}

impl Default for ThrowConfig {
    fn default() -> Self {
        Self {
            impulse: 16.0,
            angular_impulse: 2.4,
            upward_bias: 0.08,
            inherit_actor_velocity: true,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Default)]
#[reflect(Resource, Debug, PartialEq)]
pub struct ObjectInteractionConfig {
    pub acquisition: AcquisitionConfig,
    pub scoring: TargetScoringConfig,
    pub hold: HoldConfig,
    pub throw: ThrowConfig,
}
