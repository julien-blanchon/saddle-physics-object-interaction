use std::{f32::consts::PI, sync::Arc};

use avian3d::prelude::{
    ColliderOf, Forces, LinearVelocity, ReadRigidBodyForces, SpatialQuery, WriteRigidBodyForces,
};
use bevy::{ecs::relationship::Relationship, prelude::*};

use crate::{
    components::{
        HeldBy, HeldRuntime, Holding, InteractionAnchor, ObjectInteractionCommandState,
        ObjectInteractor, SurfacePlacementMode, ThrowResponseOverride,
    },
    config::{ObjectInteractionConfig, ThrowConfig},
    debug,
    messages::{HeldObjectBecameUnstable, ObjectReleased, ObjectThrown, ReleaseReason},
    systems,
};

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ReleaseEvaluation {
    pub reason: Option<ReleaseReason>,
    pub became_unstable: bool,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ThrowProfileContext {
    pub interactor: Entity,
    pub object: Entity,
    pub actor_forward: Vec3,
    pub actor_up: Vec3,
    pub actor_right: Vec3,
    pub actor_velocity: Vec3,
    pub impulse_scale: f32,
    pub angular_impulse_scale: f32,
    pub config: ThrowConfig,
    pub response: ThrowResponseOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Default)]
#[reflect(Debug, PartialEq, Default)]
pub struct ThrowImpulse {
    pub linear: Vec3,
    pub angular: Vec3,
}

pub trait ThrowProfile: Send + Sync + 'static {
    fn impulses(&self, context: &ThrowProfileContext) -> ThrowImpulse;
}

#[derive(Resource, Clone)]
pub struct ThrowProfileProvider {
    profile: Arc<dyn ThrowProfile>,
}

impl ThrowProfileProvider {
    pub fn from_profile(profile: impl ThrowProfile) -> Self {
        Self {
            profile: Arc::new(profile),
        }
    }

    pub fn from_callback(
        callback: impl Fn(&ThrowProfileContext) -> ThrowImpulse + Send + Sync + 'static,
    ) -> Self {
        Self::from_profile(CallbackThrowProfile { callback })
    }

    pub fn impulses(&self, context: &ThrowProfileContext) -> ThrowImpulse {
        self.profile.impulses(context)
    }
}

struct CallbackThrowProfile<F> {
    callback: F,
}

impl<F> ThrowProfile for CallbackThrowProfile<F>
where
    F: Fn(&ThrowProfileContext) -> ThrowImpulse + Send + Sync + 'static,
{
    fn impulses(&self, context: &ThrowProfileContext) -> ThrowImpulse {
        (self.callback)(context)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultThrowProfile;

impl ThrowProfile for DefaultThrowProfile {
    fn impulses(&self, context: &ThrowProfileContext) -> ThrowImpulse {
        ThrowImpulse {
            linear: compute_throw_impulse(
                context.actor_forward,
                context.actor_velocity,
                context.config.impulse * context.impulse_scale,
                context.config.upward_bias,
                context.config.inherit_actor_velocity,
                &context.response,
            ),
            angular: context.actor_right
                * context.config.angular_impulse
                * context.angular_impulse_scale
                * context.response.angular_impulse_scale,
        }
    }
}

pub(crate) fn compute_spring_force(
    position_error: Vec3,
    relative_velocity: Vec3,
    stiffness: f32,
    damping: f32,
    max_force: f32,
) -> Vec3 {
    (position_error * stiffness - relative_velocity * damping).clamp_length_max(max_force)
}

pub(crate) fn compute_align_torque(
    current_rotation: Quat,
    target_rotation: Quat,
    angular_velocity: Vec3,
    stiffness: f32,
    damping: f32,
    max_torque: f32,
) -> Vec3 {
    let delta = target_rotation * current_rotation.inverse();
    let (axis, raw_angle) = delta.to_axis_angle();
    let angle = if raw_angle > PI {
        raw_angle - (PI * 2.0)
    } else {
        raw_angle
    };
    (axis * angle * stiffness - angular_velocity * damping).clamp_length_max(max_torque)
}

pub(crate) fn compute_throw_impulse(
    forward: Vec3,
    actor_velocity: Vec3,
    base_impulse: f32,
    upward_bias: f32,
    inherit_actor_velocity: bool,
    response: &ThrowResponseOverride,
) -> Vec3 {
    let mut impulse = forward.normalize_or_zero() * base_impulse * response.impulse_scale;
    impulse.y += base_impulse * upward_bias * response.upward_bias_scale;
    if inherit_actor_velocity && response.inherit_actor_velocity.unwrap_or(true) {
        impulse += actor_velocity;
    }
    impulse
}

pub(crate) fn evaluate_release(
    linear_error: f32,
    unstable_seconds: f32,
    occluded_seconds: f32,
    config: &ObjectInteractionConfig,
) -> ReleaseEvaluation {
    if linear_error >= config.hold.break_distance {
        return ReleaseEvaluation {
            reason: Some(ReleaseReason::DistanceExceeded),
            became_unstable: false,
        };
    }

    if unstable_seconds >= config.hold.instability_grace_seconds {
        return ReleaseEvaluation {
            reason: Some(ReleaseReason::Unstable),
            became_unstable: true,
        };
    }

    if occluded_seconds >= config.hold.occlusion_grace_seconds {
        return ReleaseEvaluation {
            reason: Some(ReleaseReason::Occluded),
            became_unstable: false,
        };
    }

    ReleaseEvaluation {
        reason: None,
        became_unstable: false,
    }
}

pub(crate) fn world_point(position: Vec3, rotation: Quat, local_point: Vec3) -> Vec3 {
    position + rotation * local_point
}

pub(crate) fn desired_hold_rotation(actor_rotation: Quat, runtime: &HeldRuntime) -> Quat {
    actor_rotation * runtime.base_rotation_offset * runtime.rotation_adjustment
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SurfacePlacementTarget {
    pub position: Vec3,
    pub frame_rotation: Quat,
}

pub(crate) fn pull_to_hand_distance(
    start_distance: f32,
    target_distance: f32,
    elapsed: f32,
    duration: f32,
) -> f32 {
    if duration <= f32::EPSILON {
        return target_distance;
    }

    let t = (elapsed / duration).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powi(3);
    start_distance.lerp(target_distance, eased)
}

pub(crate) fn pull_to_hand_arc_height(elapsed: f32, duration: f32, max_height: f32) -> f32 {
    if duration <= f32::EPSILON || max_height <= f32::EPSILON {
        return 0.0;
    }

    let t = (elapsed / duration).clamp(0.0, 1.0);
    (t * PI).sin() * max_height
}

pub(crate) fn placement_frame_rotation(surface_normal: Vec3, reference_forward: Vec3) -> Quat {
    let up = surface_normal.try_normalize().unwrap_or(Vec3::Y);
    let mut forward = reference_forward - up * reference_forward.dot(up);
    if forward.length_squared() <= 1e-6 {
        forward = up.any_orthonormal_vector();
    }

    Transform::IDENTITY
        .looking_to(forward.normalize_or_zero(), up)
        .rotation
}

fn sample_surface_placement_target(
    spatial_query: &SpatialQuery,
    q_collider_parent: &Query<&ColliderOf>,
    interactor: &ObjectInteractor,
    actor: Entity,
    prop: Entity,
    actor_origin: Vec3,
    forward: Dir3,
    config: &ObjectInteractionConfig,
) -> Option<SurfacePlacementTarget> {
    let mut filter = interactor.obstruction_filter.clone();
    filter.excluded_entities.insert(actor);
    filter.excluded_entities.insert(prop);

    let shape = avian3d::prelude::Collider::sphere(config.hold.surface_placement.probe_radius);
    let cast_config = avian3d::prelude::ShapeCastConfig::from_max_distance(
        config.hold.surface_placement.max_distance,
    );

    spatial_query
        .shape_hits(
            &shape,
            actor_origin,
            Quat::IDENTITY,
            forward,
            16,
            &cast_config,
            &filter,
        )
        .into_iter()
        .find_map(|hit| {
            let hit_body = q_collider_parent
                .get(hit.entity)
                .map(|parent| parent.get())
                .unwrap_or(hit.entity);
            if hit_body == actor || hit_body == prop {
                return None;
            }

            let normal = hit.normal1.normalize_or_zero();
            Some(SurfacePlacementTarget {
                position: hit.point1 + normal * config.hold.surface_placement.surface_offset,
                frame_rotation: placement_frame_rotation(normal, *forward),
            })
        })
}

pub(crate) fn maintain_holds(
    time: Res<Time>,
    config: Res<ObjectInteractionConfig>,
    spatial_query: SpatialQuery,
    q_collider_parent: Query<&ColliderOf>,
    mut held_unstable: MessageWriter<HeldObjectBecameUnstable>,
    mut q_actor: Query<(
        Entity,
        &GlobalTransform,
        &ObjectInteractor,
        &InteractionAnchor,
        &crate::components::HoldDistance,
        &SurfacePlacementMode,
        &Holding,
        &mut ObjectInteractionCommandState,
    )>,
    mut q_prop: Query<(Forces, &mut HeldRuntime, &HeldBy)>,
) {
    let dt = time.delta_secs().max(1.0 / 240.0);

    for (
        actor,
        actor_transform,
        interactor,
        anchor,
        hold_distance,
        placement_mode,
        holding,
        mut command_state,
    ) in &mut q_actor
    {
        if command_state.pending_release.is_some() || command_state.pending_throw.is_some() {
            command_state.rotation_delta = Quat::IDENTITY;
            continue;
        }

        let prop = holding.0;
        let Ok((mut forces, mut runtime, held_by)) = q_prop.get_mut(prop) else {
            command_state.pending_release = Some(ReleaseReason::TargetInvalid);
            continue;
        };

        if held_by.0 != actor {
            command_state.pending_release = Some(ReleaseReason::TargetInvalid);
            continue;
        }

        runtime.rotation_adjustment = command_state.rotation_delta * runtime.rotation_adjustment;
        command_state.rotation_delta = Quat::IDENTITY;

        let actor_transform = actor_transform.compute_transform();
        let actor_origin = actor_transform.transform_point(anchor.local_offset);
        let placement_target = placement_mode
            .enabled
            .then(|| {
                sample_surface_placement_target(
                    &spatial_query,
                    &q_collider_parent,
                    interactor,
                    actor,
                    prop,
                    actor_origin,
                    actor_transform.forward(),
                    &config,
                )
            })
            .flatten();

        runtime.pull_target_distance = hold_distance.0;
        let actor_forward = *actor_transform.forward();
        let actor_up = *actor_transform.up();
        let (desired_position, desired_rotation) = if let Some(placement_target) = placement_target
        {
            runtime.pull_elapsed = runtime.pull_duration;
            let rotation_basis = if config.hold.surface_placement.align_to_surface {
                placement_target.frame_rotation
            } else {
                actor_transform.rotation
            };
            (
                placement_target.position,
                rotation_basis * runtime.base_rotation_offset * runtime.rotation_adjustment,
            )
        } else {
            runtime.pull_elapsed = (runtime.pull_elapsed + dt).min(runtime.pull_duration);
            let pull_distance = pull_to_hand_distance(
                runtime.pull_start_distance.max(hold_distance.0),
                hold_distance.0,
                runtime.pull_elapsed,
                runtime.pull_duration,
            );
            let pull_arc = pull_to_hand_arc_height(
                runtime.pull_elapsed,
                runtime.pull_duration,
                config.hold.pull_to_hand.arc_height,
            );
            (
                actor_origin + actor_forward * pull_distance + actor_up * pull_arc,
                desired_hold_rotation(actor_transform.rotation, &runtime),
            )
        };

        let body_position = forces.position().0;
        let body_rotation = forces.rotation().0;
        let anchor_world = world_point(body_position, body_rotation, runtime.local_anchor);
        let anchor_velocity = forces.velocity_at_point(anchor_world);
        let target_velocity = (desired_position - runtime.last_target_position) / dt;
        let position_error = desired_position - anchor_world;
        let relative_velocity = anchor_velocity - target_velocity;

        let force = compute_spring_force(
            position_error,
            relative_velocity,
            config.hold.linear_stiffness,
            config.hold.linear_damping,
            config.hold.max_force,
        );
        forces.apply_force_at_point(force, anchor_world);

        let torque = compute_align_torque(
            body_rotation,
            desired_rotation,
            forces.angular_velocity(),
            config.hold.angular_stiffness,
            config.hold.angular_damping,
            config.hold.max_torque,
        );
        forces.apply_torque(torque);

        let mut blocked = false;
        if config.acquisition.require_line_of_sight {
            let to_anchor = anchor_world - actor_origin;
            let distance = to_anchor.length();
            if let Ok(direction) = Dir3::new(to_anchor) {
                let mut filter = interactor.obstruction_filter.clone();
                filter.excluded_entities.insert(actor);
                if let Some(hit) =
                    spatial_query.cast_ray(actor_origin, direction, distance, true, &filter)
                {
                    let hit_body = q_collider_parent
                        .get(hit.entity)
                        .map(|parent| parent.get())
                        .unwrap_or(hit.entity);
                    if hit_body != prop {
                        blocked = true;
                    }
                }
            }
        }

        let linear_error = position_error.length();
        let was_unstable = runtime.unstable_seconds >= config.hold.instability_grace_seconds;
        runtime.unstable_seconds = if linear_error >= config.hold.instability_distance {
            runtime.unstable_seconds + dt
        } else {
            0.0
        };
        runtime.occluded_seconds = if blocked {
            runtime.occluded_seconds + dt
        } else {
            0.0
        };

        let release = evaluate_release(
            linear_error,
            runtime.unstable_seconds,
            runtime.occluded_seconds,
            &config,
        );

        if !was_unstable && release.became_unstable {
            held_unstable.write(HeldObjectBecameUnstable {
                interactor: actor,
                object: prop,
                error_distance: linear_error,
            });
        }

        if let Some(reason) = release.reason {
            command_state.pending_release = Some(reason);
        }

        runtime.last_target_position = desired_position;
        runtime.last_target_rotation = desired_rotation;
        runtime.last_force = force;
        runtime.last_torque = torque;
    }
}

pub(crate) fn release_and_throw(
    config: Res<ObjectInteractionConfig>,
    throw_profile: Option<Res<ThrowProfileProvider>>,
    mut commands: Commands,
    mut released: MessageWriter<ObjectReleased>,
    mut thrown: MessageWriter<ObjectThrown>,
    mut diagnostics: ResMut<crate::debug::ObjectInteractionDiagnostics>,
    mut q_actor: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&LinearVelocity>,
            &Holding,
            &mut ObjectInteractionCommandState,
            &mut crate::components::ObjectInteractionState,
            &mut crate::components::InteractionTarget,
            &mut crate::components::InteractionCandidates,
        ),
        Without<HeldRuntime>,
    >,
    mut q_prop: Query<(Forces, &HeldRuntime, Option<&ThrowResponseOverride>)>,
) {
    for (
        actor,
        actor_transform,
        actor_velocity,
        holding,
        mut command_state,
        mut state,
        mut target,
        mut candidates,
    ) in &mut q_actor
    {
        let prop = holding.0;
        let release_reason = command_state.pending_release.take();
        let throw_request = command_state.pending_throw.take();

        if throw_request.is_none() && release_reason.is_none() {
            continue;
        }

        let Ok((mut forces, runtime, response_override)) = q_prop.get_mut(prop) else {
            commands.entity(actor).remove::<Holding>();
            *state = crate::components::ObjectInteractionState::Idle;
            *target = crate::components::InteractionTarget::default();
            candidates.ordered.clear();
            candidates.selected = None;
            continue;
        };

        let actor_transform = actor_transform.compute_transform();
        let actor_velocity = actor_velocity.map(|value| value.0).unwrap_or(Vec3::ZERO);

        if let Some(request) = throw_request {
            let response = response_override.copied().unwrap_or_default();
            let impulse = throw_profile
                .as_deref()
                .map(|profile| {
                    profile.impulses(&ThrowProfileContext {
                        interactor: actor,
                        object: prop,
                        actor_forward: *actor_transform.forward(),
                        actor_up: *actor_transform.up(),
                        actor_right: *actor_transform.right(),
                        actor_velocity,
                        impulse_scale: request.impulse_scale,
                        angular_impulse_scale: request.angular_impulse_scale,
                        config: config.throw.clone(),
                        response,
                    })
                })
                .unwrap_or_default();
            forces.apply_linear_impulse(impulse.linear);
            forces.apply_angular_impulse(impulse.angular);
            systems::restore_collision_layers(&mut commands, prop, runtime.saved_collision_layers);
            commands.entity(actor).remove::<Holding>();
            commands.entity(prop).remove::<(HeldBy, HeldRuntime)>();
            *state = crate::components::ObjectInteractionState::Idle;
            *target = crate::components::InteractionTarget::default();
            candidates.ordered.clear();
            candidates.selected = None;
            released.write(ObjectReleased {
                interactor: actor,
                object: prop,
                reason: ReleaseReason::Thrown,
            });
            thrown.write(ObjectThrown {
                interactor: actor,
                object: prop,
                impulse: impulse.linear,
            });
            debug::record_release(&mut diagnostics, actor, prop, ReleaseReason::Thrown);
            debug::record_throw(&mut diagnostics, actor, prop, impulse.linear);
            continue;
        }

        if let Some(reason) = release_reason {
            systems::restore_collision_layers(&mut commands, prop, runtime.saved_collision_layers);
            commands.entity(actor).remove::<Holding>();
            commands.entity(prop).remove::<(HeldBy, HeldRuntime)>();
            *state = crate::components::ObjectInteractionState::Idle;
            *target = crate::components::InteractionTarget::default();
            candidates.ordered.clear();
            candidates.selected = None;
            released.write(ObjectReleased {
                interactor: actor,
                object: prop,
                reason,
            });
            debug::record_release(&mut diagnostics, actor, prop, reason);
        }
    }
}

#[cfg(test)]
#[path = "physics_tests.rs"]
mod tests;
