use avian3d::prelude::{
    ColliderOf, CollisionLayers, ComputedCenterOfMass, ComputedMass, RigidBody, ShapeCastConfig,
    SpatialQuery, SpatialQueryFilter,
};
use bevy::ecs::entity::hash_set::EntityHashSet;
use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;

use crate::{
    ObjectInteractionRuntimeActive,
    components::{
        AcquisitionMode, CandidateMethod, HeldBy, HeldRuntime, HoldAnchorMode, HoldDistance,
        HoldOrientationMode, HoldPointOverride, Holding, InteractionAnchor, InteractionCandidates,
        InteractionCollisionPolicy, InteractionMassLimitOverride, InteractionTarget,
        ObjectInteractionCommandState, ObjectInteractionState, ObjectInteractor, PendingThrow,
        PreferredHoldDistance, SavedCollisionLayers, SurfacePlacementMode,
    },
    config::ObjectInteractionConfig,
    debug,
    messages::{
        AcquireFailureReason, AdjustHoldDistance, CycleDirection, CycleInteractionTarget,
        ObjectAcquired, ObjectInteractionFailed, ReleaseHeldObject, ReleaseReason,
        RotateHeldObject, SetInteractionTarget, SetSurfacePlacementMode, ThrowHeldObject,
        TryAcquireObject,
    },
    physics,
    selection::{self, CandidateScoreInput},
};

#[derive(Debug, Clone, Copy)]
struct Candidate {
    entity: Entity,
    score: f32,
    method: CandidateMethod,
    distance: f32,
    hit_point: Option<Vec3>,
}

pub(crate) fn activate_runtime(mut runtime: ResMut<ObjectInteractionRuntimeActive>) {
    runtime.0 = true;
}

pub(crate) fn deactivate_runtime(mut runtime: ResMut<ObjectInteractionRuntimeActive>) {
    runtime.0 = false;
}

pub(crate) fn runtime_is_active(runtime: Res<ObjectInteractionRuntimeActive>) -> bool {
    runtime.0
}

pub(crate) fn seed_interactor_defaults(
    config: Res<ObjectInteractionConfig>,
    mut q_interactor: Query<&mut HoldDistance, Added<ObjectInteractor>>,
) {
    for mut hold_distance in &mut q_interactor {
        if (hold_distance.0 - HoldDistance::default().0).abs() <= f32::EPSILON {
            hold_distance.0 = config.hold.default_distance;
        }
    }
}

pub(crate) fn apply_messages(
    config: Res<ObjectInteractionConfig>,
    mut try_acquire: MessageReader<TryAcquireObject>,
    mut set_target: MessageReader<SetInteractionTarget>,
    mut release: MessageReader<ReleaseHeldObject>,
    mut throw: MessageReader<ThrowHeldObject>,
    mut adjust_hold_distance: MessageReader<AdjustHoldDistance>,
    mut rotate_held: MessageReader<RotateHeldObject>,
    mut set_surface_placement: MessageReader<SetSurfacePlacementMode>,
    mut cycle_target: MessageReader<CycleInteractionTarget>,
    mut q_interactor: Query<
        (
            &mut HoldDistance,
            &mut ObjectInteractionCommandState,
            &mut SurfacePlacementMode,
        ),
        With<ObjectInteractor>,
    >,
) {
    for message in try_acquire.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.acquire_requested = true;
        }
    }

    for message in set_target.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.override_target = message.target;
        }
    }

    for message in release.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.pending_release = Some(ReleaseReason::Dropped);
            command_state.pending_throw = None;
        }
    }

    for message in throw.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.pending_throw = Some(PendingThrow {
                impulse_scale: message.impulse_scale,
                angular_impulse_scale: message.angular_impulse_scale,
            });
            command_state.pending_release = None;
        }
    }

    for message in adjust_hold_distance.read() {
        if let Ok((mut hold_distance, _, _)) = q_interactor.get_mut(message.interactor) {
            hold_distance.0 = selection::hold_distance_clamped(
                hold_distance.0 + message.delta,
                config.hold.min_distance,
                config.hold.max_distance,
            );
        }
    }

    for message in rotate_held.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.rotation_delta = message.delta * command_state.rotation_delta;
        }
    }

    for message in set_surface_placement.read() {
        if let Ok((_, _, mut placement_mode)) = q_interactor.get_mut(message.interactor) {
            placement_mode.enabled = message.enabled;
        }
    }

    for message in cycle_target.read() {
        if let Ok((_, mut command_state, _)) = q_interactor.get_mut(message.interactor) {
            command_state.cycle_steps += match message.direction {
                CycleDirection::Next => 1,
                CycleDirection::Previous => -1,
            };
        }
    }
}

pub(crate) fn refresh_candidates(
    config: Res<ObjectInteractionConfig>,
    spatial_query: SpatialQuery,
    q_collider_parent: Query<&ColliderOf>,
    q_prop: Query<(
        &crate::components::InteractableBody,
        &RigidBody,
        &ComputedMass,
        &GlobalTransform,
        Option<&InteractionMassLimitOverride>,
        Has<HeldBy>,
    )>,
    mut q_interactor: Query<(
        Entity,
        &GlobalTransform,
        &ObjectInteractor,
        &InteractionAnchor,
        &mut InteractionCandidates,
        &mut InteractionTarget,
        &mut ObjectInteractionState,
        &mut ObjectInteractionCommandState,
        Option<&Holding>,
    )>,
) {
    for (
        entity,
        actor_transform,
        interactor,
        anchor,
        mut candidates,
        mut target,
        mut state,
        mut command_state,
        holding,
    ) in &mut q_interactor
    {
        if holding.is_some() {
            continue;
        }

        if !interactor.enabled {
            candidates.ordered.clear();
            candidates.selected = None;
            *target = InteractionTarget::default();
            *state = ObjectInteractionState::Idle;
            command_state.cycle_steps = 0;
            continue;
        }

        let actor_transform = actor_transform.compute_transform();
        let origin = actor_transform.transform_point(anchor.local_offset);
        let forward = actor_transform.forward();
        command_state.last_rejected_target = None;
        let mut filter = interactor.candidate_filter.clone();
        filter.excluded_entities.insert(entity);

        let direct_hit = matches!(
            interactor.acquisition_mode,
            AcquisitionMode::RaycastOnly | AcquisitionMode::Hybrid
        )
        .then(|| {
            find_direct_hit(
                &spatial_query,
                &q_collider_parent,
                &filter,
                origin,
                forward,
                config.acquisition.max_distance,
            )
        })
        .flatten();

        let mut forgiving_hits = if matches!(
            interactor.acquisition_mode,
            AcquisitionMode::OverlapOnly | AcquisitionMode::Hybrid
        ) && config.acquisition.forgiving_radius > 0.0
        {
            find_forgiving_hits(
                &spatial_query,
                &q_collider_parent,
                &filter,
                origin,
                forward,
                config.acquisition.max_distance,
                config.acquisition.forgiving_radius,
            )
        } else {
            Vec::new()
        };
        forgiving_hits.sort_by(|left, right| left.distance.total_cmp(&right.distance));

        let mut unique_entities = EntityHashSet::default();
        let mut scored = Vec::new();

        for hit in forgiving_hits {
            if !unique_entities.insert(hit.entity) {
                continue;
            }

            let Ok((body, rigid_body, mass, body_transform, mass_override, held)) =
                q_prop.get(hit.entity)
            else {
                continue;
            };

            if !body.enabled || !rigid_body.is_dynamic() || held {
                continue;
            }

            let to_body = body_transform.translation() - origin;
            let distance = to_body.length();
            if distance > config.acquisition.max_distance {
                continue;
            }

            let Ok(direction) = Dir3::new(to_body) else {
                continue;
            };

            let alignment = direction.dot(*forward);
            if alignment < selection::cone_alignment_cutoff(&config.acquisition) {
                continue;
            }

            let effective_mass = effective_target_mass(*mass, mass_override.copied());
            let max_mass = interactor
                .max_target_mass
                .unwrap_or(config.acquisition.max_target_mass);
            if effective_mass > max_mass {
                continue;
            }

            if config.acquisition.require_line_of_sight
                && !has_line_of_sight(
                    &spatial_query,
                    &q_collider_parent,
                    interactor.obstruction_filter.clone(),
                    entity,
                    hit.entity,
                    origin,
                    body_transform.translation(),
                )
            {
                continue;
            }

            let sticky = target.entity == Some(hit.entity);
            let direct_hit_match = direct_hit
                .as_ref()
                .is_some_and(|direct_hit| direct_hit.entity == hit.entity);
            let score = selection::score_candidate(
                CandidateScoreInput {
                    distance,
                    alignment,
                    priority: body.priority,
                    direct_hit: direct_hit_match,
                    sticky,
                },
                &config.acquisition,
                &config.scoring,
            );

            scored.push(Candidate {
                entity: hit.entity,
                score,
                method: if direct_hit_match {
                    CandidateMethod::DirectHit
                } else {
                    CandidateMethod::Overlap
                },
                distance,
                hit_point: direct_hit
                    .as_ref()
                    .and_then(|direct_hit| {
                        (direct_hit.entity == hit.entity).then_some(direct_hit.point)
                    })
                    .or(Some(hit.point)),
            });
        }

        if let Some(hit) = direct_hit {
            if let Ok((body, rigid_body, mass, _, mass_override, held)) = q_prop.get(hit.entity) {
                let effective_mass = effective_target_mass(*mass, mass_override.copied());
                let max_mass = interactor
                    .max_target_mass
                    .unwrap_or(config.acquisition.max_target_mass);
                if body.enabled && rigid_body.is_dynamic() && !held && effective_mass <= max_mass {
                    if let Some(existing) = scored
                        .iter_mut()
                        .find(|candidate| candidate.entity == hit.entity)
                    {
                        existing.method = CandidateMethod::DirectHit;
                        existing.hit_point = Some(hit.point);
                    } else {
                        scored.push(Candidate {
                            entity: hit.entity,
                            score: selection::score_candidate(
                                CandidateScoreInput {
                                    distance: hit.distance,
                                    alignment: 1.0,
                                    priority: body.priority,
                                    direct_hit: true,
                                    sticky: target.entity == Some(hit.entity),
                                },
                                &config.acquisition,
                                &config.scoring,
                            ),
                            method: CandidateMethod::DirectHit,
                            distance: hit.distance,
                            hit_point: Some(hit.point),
                        });
                    }
                } else {
                    command_state.last_rejected_target = Some((
                        hit.entity,
                        if !body.enabled {
                            AcquireFailureReason::NoValidTarget
                        } else if !rigid_body.is_dynamic() {
                            AcquireFailureReason::TargetNotDynamic
                        } else if held {
                            AcquireFailureReason::TargetAlreadyHeld
                        } else {
                            AcquireFailureReason::TargetTooHeavy
                        },
                    ));
                }
            } else {
                command_state.last_rejected_target =
                    Some((hit.entity, AcquireFailureReason::NoValidTarget));
            }
        }

        scored.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.distance.total_cmp(&right.distance))
        });

        let previous_target = target.entity;
        candidates.ordered = scored.iter().map(|candidate| candidate.entity).collect();

        let selected = if let Some(override_target) = command_state.override_target {
            candidates
                .ordered
                .iter()
                .position(|candidate| *candidate == override_target)
        } else if command_state.cycle_steps != 0 {
            let base = previous_target.and_then(|previous| {
                candidates
                    .ordered
                    .iter()
                    .position(|candidate| *candidate == previous)
            });
            selection::select_index(candidates.ordered.len(), base, command_state.cycle_steps)
        } else if let Some(previous) = previous_target {
            let previous_index = candidates
                .ordered
                .iter()
                .position(|candidate| *candidate == previous)
                .or(Some(0).filter(|_| !candidates.ordered.is_empty()));

            match previous_index {
                Some(previous_index)
                    if previous_index > 0
                        && selection::should_keep_previous_target(
                            scored[0].score,
                            scored[previous_index].score,
                            config.acquisition.target_switch_hysteresis,
                        ) =>
                {
                    Some(previous_index)
                }
                Some(_) => Some(0),
                None => None,
            }
        } else {
            Some(0).filter(|_| !candidates.ordered.is_empty())
        };

        candidates.selected = selected;
        command_state.cycle_steps = 0;

        if let Some(selected) = selected {
            let chosen = scored[selected];
            target.entity = Some(chosen.entity);
            target.score = chosen.score;
            target.method = if command_state.override_target == Some(chosen.entity) {
                CandidateMethod::ExplicitTarget
            } else {
                chosen.method
            };
            target.hit_point = chosen.hit_point;
            *state = ObjectInteractionState::Targeting {
                entity: chosen.entity,
                method: target.method,
            };
        } else {
            *target = InteractionTarget::default();
            *state = ObjectInteractionState::Idle;
        }
    }
}

pub(crate) fn acquire_selected_targets(
    config: Res<ObjectInteractionConfig>,
    spatial_query: SpatialQuery,
    q_collider_parent: Query<&ColliderOf>,
    mut commands: Commands,
    mut acquired: MessageWriter<ObjectAcquired>,
    mut failed: MessageWriter<ObjectInteractionFailed>,
    mut diagnostics: ResMut<crate::debug::ObjectInteractionDiagnostics>,
    mut q_actor: Query<(
        Entity,
        &GlobalTransform,
        &ObjectInteractor,
        &InteractionAnchor,
        &mut HoldDistance,
        &mut ObjectInteractionCommandState,
        &mut InteractionTarget,
        &mut InteractionCandidates,
        &mut ObjectInteractionState,
        Option<&CollisionLayers>,
        Option<&Holding>,
    )>,
    q_prop: Query<(
        &crate::components::InteractableBody,
        &RigidBody,
        &ComputedMass,
        Option<&ComputedCenterOfMass>,
        &GlobalTransform,
        Option<&PreferredHoldDistance>,
        Option<&InteractionMassLimitOverride>,
        Option<&HoldPointOverride>,
        Option<&crate::components::HoldOrientationOverride>,
        Option<&crate::components::InteractionCollisionPolicy>,
        Option<&CollisionLayers>,
        Has<HeldBy>,
    )>,
) {
    let mut claimed_props = EntityHashSet::default();

    for (
        actor,
        actor_transform,
        interactor,
        anchor,
        mut hold_distance,
        mut command_state,
        mut target,
        mut candidates,
        mut state,
        actor_layers,
        holding,
    ) in &mut q_actor
    {
        if holding.is_some() || !command_state.acquire_requested {
            continue;
        }
        command_state.acquire_requested = false;

        if !interactor.enabled {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                target.entity,
                AcquireFailureReason::InteractorDisabled,
            );
            continue;
        }

        let selected_target = if let Some(override_target) = command_state.override_target {
            if target.entity == Some(override_target) {
                Some(override_target)
            } else {
                emit_failure(
                    &mut failed,
                    &mut diagnostics,
                    actor,
                    Some(override_target),
                    explicit_target_failure_reason(
                        &config,
                        &spatial_query,
                        &q_collider_parent,
                        actor,
                        actor_transform.compute_transform(),
                        anchor.local_offset,
                        interactor,
                        override_target,
                        &q_prop,
                        &claimed_props,
                    ),
                );
                None
            }
        } else {
            target.entity
        };

        let Some(prop) = selected_target else {
            if command_state.override_target.is_none() {
                if let Some((rejected, reason)) = command_state.last_rejected_target {
                    emit_failure(&mut failed, &mut diagnostics, actor, Some(rejected), reason);
                    continue;
                }
                emit_failure(
                    &mut failed,
                    &mut diagnostics,
                    actor,
                    None,
                    AcquireFailureReason::NoValidTarget,
                );
            }
            continue;
        };

        let Ok((
            body,
            rigid_body,
            mass,
            center_of_mass,
            prop_transform,
            preferred_distance,
            mass_override,
            hold_point_override,
            orientation_override,
            collision_policy_override,
            layers,
            is_held,
        )) = q_prop.get(prop)
        else {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::NoValidTarget,
            );
            continue;
        };

        if !body.enabled || !rigid_body.is_dynamic() {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::TargetNotDynamic,
            );
            continue;
        }

        if is_held {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::TargetAlreadyHeld,
            );
            continue;
        }

        let actual_mass_limit = effective_target_mass(*mass, mass_override.copied());
        let max_mass = interactor
            .max_target_mass
            .unwrap_or(config.acquisition.max_target_mass);
        if actual_mass_limit > max_mass {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::TargetTooHeavy,
            );
            continue;
        }

        let actor_transform = actor_transform.compute_transform();
        let origin = actor_transform.transform_point(anchor.local_offset);
        let prop_translation = prop_transform.translation();
        let prop_distance = origin.distance(prop_translation);
        if prop_distance > config.acquisition.max_distance {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::TargetTooFar,
            );
            continue;
        }

        let center_of_mass = center_of_mass.map(|value| value.0).unwrap_or(Vec3::ZERO);
        let hit_point = target.hit_point;
        let local_anchor = match body.anchor_mode {
            HoldAnchorMode::CenterOfMass => center_of_mass,
            HoldAnchorMode::HitPoint => hit_point
                .map(|point| prop_transform.affine().inverse().transform_point3(point))
                .unwrap_or(center_of_mass),
            HoldAnchorMode::CustomLocal => hold_point_override
                .map(|override_point| override_point.local_offset)
                .unwrap_or(center_of_mass),
        };

        let orientation_mode = resolve_orientation_mode(
            orientation_override.map(|value| value.mode),
            interactor.orientation_mode,
            config.hold.orientation_mode,
        );
        let base_rotation_offset = match orientation_mode {
            HoldOrientationMode::UseConfig => {
                unreachable!("UseConfig should be resolved before calculating hold rotation")
            }
            HoldOrientationMode::PreserveWorld => {
                actor_transform.rotation.inverse() * prop_transform.compute_transform().rotation
            }
            HoldOrientationMode::AlignToInteractor => Quat::IDENTITY,
            HoldOrientationMode::CustomLocal => hold_point_override
                .map(|override_point| override_point.local_rotation)
                .unwrap_or(Quat::IDENTITY),
        };

        hold_distance.0 = selection::hold_distance_clamped(
            preferred_distance
                .map(|distance| distance.0)
                .unwrap_or(hold_distance.0),
            config.hold.min_distance,
            config.hold.max_distance,
        );

        if !claimed_props.insert(prop) {
            emit_failure(
                &mut failed,
                &mut diagnostics,
                actor,
                Some(prop),
                AcquireFailureReason::TargetAlreadyHeld,
            );
            continue;
        }

        let desired_position = origin + actor_transform.forward() * hold_distance.0;
        let mut held_runtime = HeldRuntime::new(
            local_anchor,
            base_rotation_offset,
            desired_position,
            Quat::IDENTITY,
            None,
        );
        let prop_distance = origin.distance(prop_translation);
        if config.hold.pull_to_hand.enabled
            && prop_distance >= hold_distance.0 + config.hold.pull_to_hand.min_start_distance
        {
            held_runtime.pull_duration = config.hold.pull_to_hand.duration_seconds.max(0.0);
            held_runtime.pull_start_distance = prop_distance.max(hold_distance.0);
            held_runtime.pull_target_distance = hold_distance.0;
        }
        let desired_rotation =
            physics::desired_hold_rotation(actor_transform.rotation, &held_runtime);
        let collision_policy = collision_policy_override
            .copied()
            .unwrap_or(config.hold.collision_policy);
        let saved_collision_layers = apply_collision_policy(
            &mut commands,
            prop,
            layers.copied(),
            collision_policy,
            actor_layers.copied(),
        );

        commands.entity(actor).insert(Holding(prop));
        commands.entity(prop).insert((
            HeldBy(actor),
            HeldRuntime {
                last_target_rotation: desired_rotation,
                saved_collision_layers,
                ..held_runtime
            },
        ));
        *state = ObjectInteractionState::Holding(prop);
        *target = InteractionTarget::default();
        candidates.ordered.clear();
        candidates.selected = None;
        command_state.rotation_delta = Quat::IDENTITY;

        acquired.write(ObjectAcquired {
            interactor: actor,
            object: prop,
        });
        debug::record_acquire(&mut diagnostics, actor, prop);
    }
}

pub(crate) fn release_all_holds(
    mut commands: Commands,
    mut released: MessageWriter<crate::messages::ObjectReleased>,
    mut diagnostics: ResMut<crate::debug::ObjectInteractionDiagnostics>,
    mut q_actor: Query<(
        Entity,
        &Holding,
        &mut ObjectInteractionState,
        &mut InteractionTarget,
        &mut InteractionCandidates,
    )>,
    q_prop: Query<&HeldRuntime>,
) {
    for (actor, holding, mut state, mut target, mut candidates) in &mut q_actor {
        let prop = holding.0;
        if let Ok(runtime) = q_prop.get(prop) {
            restore_collision_layers(&mut commands, prop, runtime.saved_collision_layers);
            commands.entity(prop).remove::<(HeldBy, HeldRuntime)>();
        }
        commands.entity(actor).remove::<Holding>();
        *state = ObjectInteractionState::Idle;
        *target = InteractionTarget::default();
        candidates.ordered.clear();
        candidates.selected = None;
        released.write(crate::messages::ObjectReleased {
            interactor: actor,
            object: prop,
            reason: ReleaseReason::Deactivated,
        });
        debug::record_release(&mut diagnostics, actor, prop, ReleaseReason::Deactivated);
    }
}

#[derive(Debug, Clone, Copy)]
struct DirectHit {
    entity: Entity,
    distance: f32,
    point: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct ForgivingHit {
    entity: Entity,
    distance: f32,
    point: Vec3,
}

fn find_direct_hit(
    spatial_query: &SpatialQuery,
    q_collider_parent: &Query<&ColliderOf>,
    filter: &SpatialQueryFilter,
    origin: Vec3,
    forward: Dir3,
    max_distance: f32,
) -> Option<DirectHit> {
    spatial_query
        .cast_ray(origin, forward, max_distance, true, filter)
        .map(|hit| {
            let entity = q_collider_parent
                .get(hit.entity)
                .map(|parent| parent.get())
                .unwrap_or(hit.entity);
            DirectHit {
                entity,
                distance: hit.distance,
                point: origin + *forward * hit.distance,
            }
        })
}

fn find_forgiving_hits(
    spatial_query: &SpatialQuery,
    q_collider_parent: &Query<&ColliderOf>,
    filter: &SpatialQueryFilter,
    origin: Vec3,
    forward: Dir3,
    max_distance: f32,
    radius: f32,
) -> Vec<ForgivingHit> {
    let shape = avian3d::prelude::Collider::sphere(radius);
    let config = ShapeCastConfig::from_max_distance(max_distance);

    spatial_query
        .shape_hits(&shape, origin, Quat::IDENTITY, forward, 32, &config, filter)
        .into_iter()
        .map(|hit| {
            let entity = q_collider_parent
                .get(hit.entity)
                .map(|parent| parent.get())
                .unwrap_or(hit.entity);
            ForgivingHit {
                entity,
                distance: hit.distance,
                point: hit.point1,
            }
        })
        .collect()
}

fn has_line_of_sight(
    spatial_query: &SpatialQuery,
    q_collider_parent: &Query<&ColliderOf>,
    mut filter: SpatialQueryFilter,
    actor: Entity,
    target: Entity,
    origin: Vec3,
    target_position: Vec3,
) -> bool {
    filter.excluded_entities.insert(actor);
    let to_target = target_position - origin;
    let distance = to_target.length();
    let Ok(direction) = Dir3::new(to_target) else {
        return true;
    };

    let Some(hit) = spatial_query.cast_ray(origin, direction, distance, true, &filter) else {
        return true;
    };

    let hit_body = q_collider_parent
        .get(hit.entity)
        .map(|parent| parent.get())
        .unwrap_or(hit.entity);
    hit_body == target
}

fn effective_target_mass(
    mass: ComputedMass,
    override_mass: Option<InteractionMassLimitOverride>,
) -> f32 {
    override_mass
        .map(|value| value.0)
        .filter(|value| *value > 0.0)
        .unwrap_or(mass.value())
}

fn resolve_orientation_mode(
    override_mode: Option<HoldOrientationMode>,
    interactor_mode: HoldOrientationMode,
    default_mode: HoldOrientationMode,
) -> HoldOrientationMode {
    let default_mode = match default_mode {
        HoldOrientationMode::UseConfig => HoldOrientationMode::PreserveWorld,
        mode => mode,
    };

    match override_mode {
        Some(HoldOrientationMode::UseConfig) | None => match interactor_mode {
            HoldOrientationMode::UseConfig => default_mode,
            mode => mode,
        },
        Some(mode) => mode,
    }
}

fn explicit_target_failure_reason(
    config: &ObjectInteractionConfig,
    spatial_query: &SpatialQuery,
    q_collider_parent: &Query<&ColliderOf>,
    actor: Entity,
    actor_transform: Transform,
    anchor_local_offset: Vec3,
    interactor: &ObjectInteractor,
    target: Entity,
    q_prop: &Query<(
        &crate::components::InteractableBody,
        &RigidBody,
        &ComputedMass,
        Option<&ComputedCenterOfMass>,
        &GlobalTransform,
        Option<&PreferredHoldDistance>,
        Option<&InteractionMassLimitOverride>,
        Option<&HoldPointOverride>,
        Option<&crate::components::HoldOrientationOverride>,
        Option<&crate::components::InteractionCollisionPolicy>,
        Option<&CollisionLayers>,
        Has<HeldBy>,
    )>,
    claimed_props: &EntityHashSet,
) -> AcquireFailureReason {
    let Ok((body, rigid_body, mass, _, prop_transform, _, mass_override, _, _, _, _, is_held)) =
        q_prop.get(target)
    else {
        return AcquireFailureReason::InvalidExplicitTarget;
    };

    if !body.enabled {
        return AcquireFailureReason::NoValidTarget;
    }
    if !rigid_body.is_dynamic() {
        return AcquireFailureReason::TargetNotDynamic;
    }
    if is_held || claimed_props.contains(&target) {
        return AcquireFailureReason::TargetAlreadyHeld;
    }

    let effective_mass = effective_target_mass(*mass, mass_override.copied());
    let max_mass = interactor
        .max_target_mass
        .unwrap_or(config.acquisition.max_target_mass);
    if effective_mass > max_mass {
        return AcquireFailureReason::TargetTooHeavy;
    }

    let origin = actor_transform.transform_point(anchor_local_offset);
    let prop_translation = prop_transform.translation();
    if origin.distance(prop_translation) > config.acquisition.max_distance {
        return AcquireFailureReason::TargetTooFar;
    }

    if config.acquisition.require_line_of_sight
        && !has_line_of_sight(
            spatial_query,
            q_collider_parent,
            interactor.obstruction_filter.clone(),
            actor,
            target,
            origin,
            prop_translation,
        )
    {
        return AcquireFailureReason::TargetBlocked;
    }

    AcquireFailureReason::InvalidExplicitTarget
}

fn emit_failure(
    failed: &mut MessageWriter<ObjectInteractionFailed>,
    diagnostics: &mut crate::debug::ObjectInteractionDiagnostics,
    actor: Entity,
    target: Option<Entity>,
    reason: AcquireFailureReason,
) {
    failed.write(ObjectInteractionFailed {
        interactor: actor,
        target,
        reason,
    });
    debug::record_failure(diagnostics, actor, target, reason);
}

pub(crate) fn apply_collision_policy(
    commands: &mut Commands,
    entity: Entity,
    layers: Option<CollisionLayers>,
    policy: InteractionCollisionPolicy,
    actor_layers: Option<CollisionLayers>,
) -> Option<SavedCollisionLayers> {
    let original = layers;
    match policy {
        InteractionCollisionPolicy::Preserve => None,
        InteractionCollisionPolicy::IgnoreInteractorLayer => {
            if let Some(actor_layers) = actor_layers {
                let mut next = layers.unwrap_or(CollisionLayers::DEFAULT);
                next.filters.remove(actor_layers.memberships);
                commands.entity(entity).insert(next);
                Some(original.map_or(SavedCollisionLayers::Absent, SavedCollisionLayers::Present))
            } else {
                None
            }
        }
        InteractionCollisionPolicy::DisableAll => {
            commands.entity(entity).insert(CollisionLayers::NONE);
            Some(original.map_or(SavedCollisionLayers::Absent, SavedCollisionLayers::Present))
        }
        InteractionCollisionPolicy::CustomLayers(custom) => {
            commands.entity(entity).insert(custom);
            Some(original.map_or(SavedCollisionLayers::Absent, SavedCollisionLayers::Present))
        }
    }
}

pub(crate) fn restore_collision_layers(
    commands: &mut Commands,
    entity: Entity,
    original: Option<SavedCollisionLayers>,
) {
    match original {
        Some(SavedCollisionLayers::Absent) => {
            commands.entity(entity).remove::<CollisionLayers>();
        }
        Some(SavedCollisionLayers::Present(layers)) => {
            commands.entity(entity).insert(layers);
        }
        None => {}
    }
}

#[cfg(test)]
#[path = "systems_tests.rs"]
mod tests;
