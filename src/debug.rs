use bevy::prelude::*;

use crate::components::{
    HeldRuntime, Holding, InteractionCandidates, InteractionTarget, ObjectInteractionState,
};

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Default)]
#[reflect(Resource)]
pub struct ObjectInteractionDebugSettings {
    pub enabled: bool,
    pub draw_gizmos: bool,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct InteractionDiagnosticEntry {
    pub interactor: Entity,
    pub target: Option<Entity>,
    pub held_object: Option<Entity>,
    pub hold_distance: f32,
    pub candidate_count: usize,
    pub last_force: Vec3,
    pub last_torque: Vec3,
    pub unstable_seconds: f32,
    pub occluded_seconds: f32,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ObjectInteractionFailureRecord {
    pub interactor: Entity,
    pub target: Option<Entity>,
    pub reason: crate::messages::AcquireFailureReason,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ObjectInteractionReleaseRecord {
    pub interactor: Entity,
    pub object: Entity,
    pub reason: crate::messages::ReleaseReason,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ObjectInteractionThrowRecord {
    pub interactor: Entity,
    pub object: Entity,
    pub impulse: Vec3,
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Default)]
#[reflect(Resource)]
pub struct ObjectInteractionDiagnostics {
    pub interactors: Vec<InteractionDiagnosticEntry>,
    pub acquisition_count: u32,
    pub release_count: u32,
    pub throw_count: u32,
    pub last_failure: Option<ObjectInteractionFailureRecord>,
    pub last_release: Option<ObjectInteractionReleaseRecord>,
    pub last_throw: Option<ObjectInteractionThrowRecord>,
}

pub(crate) fn record_acquire(
    diagnostics: &mut ObjectInteractionDiagnostics,
    interactor: Entity,
    object: Entity,
) {
    diagnostics.acquisition_count += 1;
    if let Some(entry) = diagnostics
        .interactors
        .iter_mut()
        .find(|entry| entry.interactor == interactor)
    {
        entry.held_object = Some(object);
    }
}

pub(crate) fn record_failure(
    diagnostics: &mut ObjectInteractionDiagnostics,
    interactor: Entity,
    target: Option<Entity>,
    reason: crate::messages::AcquireFailureReason,
) {
    diagnostics.last_failure = Some(ObjectInteractionFailureRecord {
        interactor,
        target,
        reason,
    });
}

pub(crate) fn record_release(
    diagnostics: &mut ObjectInteractionDiagnostics,
    interactor: Entity,
    object: Entity,
    reason: crate::messages::ReleaseReason,
) {
    diagnostics.release_count += 1;
    diagnostics.last_release = Some(ObjectInteractionReleaseRecord {
        interactor,
        object,
        reason,
    });
}

pub(crate) fn record_throw(
    diagnostics: &mut ObjectInteractionDiagnostics,
    interactor: Entity,
    object: Entity,
    impulse: Vec3,
) {
    diagnostics.throw_count += 1;
    diagnostics.last_throw = Some(ObjectInteractionThrowRecord {
        interactor,
        object,
        impulse,
    });
}

pub(crate) fn refresh_diagnostics(
    mut diagnostics: ResMut<ObjectInteractionDiagnostics>,
    q_interactor: Query<(
        Entity,
        &crate::components::HoldDistance,
        &InteractionCandidates,
        &InteractionTarget,
        &ObjectInteractionState,
        Option<&Holding>,
    )>,
    q_held_runtime: Query<&HeldRuntime>,
) {
    diagnostics.interactors.clear();

    for (entity, hold_distance, candidates, target, state, holding) in &q_interactor {
        let (last_force, last_torque, unstable_seconds, occluded_seconds) = holding
            .and_then(|holding| q_held_runtime.get(holding.0).ok())
            .map(|runtime| {
                (
                    runtime.last_force,
                    runtime.last_torque,
                    runtime.unstable_seconds,
                    runtime.occluded_seconds,
                )
            })
            .unwrap_or((Vec3::ZERO, Vec3::ZERO, 0.0, 0.0));

        let held_object = match *state {
            ObjectInteractionState::Holding(object) => Some(object),
            _ => None,
        };

        diagnostics.interactors.push(InteractionDiagnosticEntry {
            interactor: entity,
            target: target.entity,
            held_object,
            hold_distance: hold_distance.0,
            candidate_count: candidates.ordered.len(),
            last_force,
            last_torque,
            unstable_seconds,
            occluded_seconds,
        });
    }
}

pub(crate) fn debug_gizmos_enabled(settings: Res<ObjectInteractionDebugSettings>) -> bool {
    settings.enabled && settings.draw_gizmos
}

pub(crate) fn draw_debug(
    mut gizmos: Gizmos,
    q_interactor: Query<(
        &GlobalTransform,
        &crate::components::InteractionAnchor,
        &InteractionTarget,
        Option<&Holding>,
    )>,
    q_prop: Query<&GlobalTransform>,
) {
    for (actor_transform, anchor, target, holding) in &q_interactor {
        let actor_transform = actor_transform.compute_transform();
        let origin = actor_transform.transform_point(anchor.local_offset);
        let forward = origin + actor_transform.forward() * 1.0;
        gizmos.line(origin, forward, Color::srgb(0.2, 0.9, 1.0));

        if let Some(target_entity) = target.entity {
            if let Ok(target_transform) = q_prop.get(target_entity) {
                gizmos.line(
                    origin,
                    target_transform.translation(),
                    Color::srgb(1.0, 0.8, 0.2),
                );
            }
        }

        if let Some(holding) = holding {
            if let Ok(prop_transform) = q_prop.get(holding.0) {
                gizmos.line(
                    origin,
                    prop_transform.translation(),
                    Color::srgb(0.1, 1.0, 0.35),
                );
            }
        }
    }
}
