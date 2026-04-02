//! Message-driven 3D object interaction for Avian-backed rigid bodies.

mod components;
mod config;
mod debug;
mod messages;
mod physics;
mod selection;
mod systems;

pub use components::{
    AcquisitionMode, CandidateMethod, HeldBy, HoldAnchorMode, HoldDistance, HoldOrientationMode,
    HoldOrientationOverride, HoldPointOverride, Holding, InteractableBody, InteractionAnchor,
    InteractionCandidates, InteractionCollisionPolicy, InteractionMassLimitOverride,
    InteractionTarget, ObjectInteractionState, ObjectInteractor, PreferredHoldDistance,
    ThrowResponseOverride,
};
pub use config::{
    AcquisitionConfig, HoldConfig, ObjectInteractionConfig, TargetScoringConfig, ThrowConfig,
};
pub use debug::{
    InteractionDiagnosticEntry, ObjectInteractionDebugSettings, ObjectInteractionDiagnostics,
    ObjectInteractionFailureRecord, ObjectInteractionReleaseRecord, ObjectInteractionThrowRecord,
};
pub use messages::{
    AcquireFailureReason, AdjustHoldDistance, CycleDirection, CycleInteractionTarget,
    HeldObjectBecameUnstable, ObjectAcquired, ObjectInteractionFailed, ObjectReleased,
    ObjectThrown, ReleaseHeldObject, ReleaseReason, RotateHeldObject, SetInteractionTarget,
    ThrowHeldObject, TryAcquireObject,
};

use avian3d::prelude::{PhysicsSchedule, PhysicsStepSystems};
use bevy::{
    app::PostStartup,
    ecs::{intern::Interned, schedule::ScheduleLabel},
    gizmos::config::GizmoConfigStore,
    prelude::*,
};

#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ObjectInteractionSystems {
    ReadCommands,
    RefreshCandidates,
    AcquireTargets,
    MaintainHold,
    ReleaseAndThrow,
    Presentation,
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct NeverDeactivateSchedule;

#[derive(Resource, Default)]
struct ObjectInteractionRuntimeActive(bool);

pub struct ObjectInteractionPlugin {
    pub activate_schedule: Interned<dyn ScheduleLabel>,
    pub deactivate_schedule: Interned<dyn ScheduleLabel>,
    pub update_schedule: Interned<dyn ScheduleLabel>,
    pub physics_schedule: Interned<dyn ScheduleLabel>,
    pub config: ObjectInteractionConfig,
}

impl ObjectInteractionPlugin {
    pub fn new(
        activate_schedule: impl ScheduleLabel,
        deactivate_schedule: impl ScheduleLabel,
        update_schedule: impl ScheduleLabel,
        physics_schedule: impl ScheduleLabel,
    ) -> Self {
        Self {
            activate_schedule: activate_schedule.intern(),
            deactivate_schedule: deactivate_schedule.intern(),
            update_schedule: update_schedule.intern(),
            physics_schedule: physics_schedule.intern(),
            config: ObjectInteractionConfig::default(),
        }
    }

    pub fn always_on(
        update_schedule: impl ScheduleLabel,
        physics_schedule: impl ScheduleLabel,
    ) -> Self {
        Self::new(
            PostStartup,
            NeverDeactivateSchedule,
            update_schedule,
            physics_schedule,
        )
    }

    pub fn with_config(mut self, config: ObjectInteractionConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for ObjectInteractionPlugin {
    fn default() -> Self {
        Self::always_on(Update, PhysicsSchedule)
    }
}

impl Plugin for ObjectInteractionPlugin {
    fn build(&self, app: &mut App) {
        if self.deactivate_schedule == NeverDeactivateSchedule.intern() {
            app.init_schedule(NeverDeactivateSchedule);
        }

        if self.physics_schedule == PhysicsSchedule.intern()
            && app.get_schedule(PhysicsSchedule).is_none()
        {
            panic!(
                "Failed to build `ObjectInteractionPlugin`: Avian's `PhysicsSchedule` was not found. \
Add Avian's physics plugins before `ObjectInteractionPlugin`, or pass a custom fixed-step schedule."
            );
        }

        if !app.world().contains_resource::<ObjectInteractionConfig>() {
            app.insert_resource(self.config.clone());
        }

        app.init_resource::<ObjectInteractionRuntimeActive>()
            .init_resource::<ObjectInteractionDiagnostics>()
            .init_resource::<ObjectInteractionDebugSettings>()
            .add_message::<TryAcquireObject>()
            .add_message::<SetInteractionTarget>()
            .add_message::<ReleaseHeldObject>()
            .add_message::<ThrowHeldObject>()
            .add_message::<AdjustHoldDistance>()
            .add_message::<RotateHeldObject>()
            .add_message::<CycleInteractionTarget>()
            .add_message::<ObjectAcquired>()
            .add_message::<ObjectReleased>()
            .add_message::<ObjectThrown>()
            .add_message::<ObjectInteractionFailed>()
            .add_message::<HeldObjectBecameUnstable>()
            .register_type::<AcquisitionConfig>()
            .register_type::<AcquisitionMode>()
            .register_type::<AcquireFailureReason>()
            .register_type::<CandidateMethod>()
            .register_type::<CycleDirection>()
            .register_type::<HeldBy>()
            .register_type::<HoldAnchorMode>()
            .register_type::<HoldConfig>()
            .register_type::<HoldDistance>()
            .register_type::<HoldOrientationMode>()
            .register_type::<HoldOrientationOverride>()
            .register_type::<HoldPointOverride>()
            .register_type::<Holding>()
            .register_type::<InteractableBody>()
            .register_type::<InteractionAnchor>()
            .register_type::<InteractionCandidates>()
            .register_type::<InteractionCollisionPolicy>()
            .register_type::<InteractionDiagnosticEntry>()
            .register_type::<InteractionMassLimitOverride>()
            .register_type::<InteractionTarget>()
            .register_type::<ObjectAcquired>()
            .register_type::<ObjectInteractionConfig>()
            .register_type::<ObjectInteractionDebugSettings>()
            .register_type::<ObjectInteractionDiagnostics>()
            .register_type::<ObjectInteractionFailureRecord>()
            .register_type::<ObjectInteractionFailed>()
            .register_type::<ObjectInteractionReleaseRecord>()
            .register_type::<ObjectInteractionState>()
            .register_type::<ObjectInteractionThrowRecord>()
            .register_type::<ObjectInteractor>()
            .register_type::<ObjectReleased>()
            .register_type::<ObjectThrown>()
            .register_type::<PreferredHoldDistance>()
            .register_type::<ReleaseReason>()
            .register_type::<TargetScoringConfig>()
            .register_type::<ThrowConfig>()
            .register_type::<ThrowResponseOverride>()
            .configure_sets(
                self.update_schedule,
                (
                    ObjectInteractionSystems::ReadCommands,
                    ObjectInteractionSystems::RefreshCandidates,
                    ObjectInteractionSystems::AcquireTargets,
                    ObjectInteractionSystems::Presentation,
                )
                    .chain(),
            )
            .add_systems(self.activate_schedule, systems::activate_runtime)
            .add_systems(
                self.deactivate_schedule,
                (systems::release_all_holds, systems::deactivate_runtime).chain(),
            )
            .add_systems(
                self.update_schedule,
                (systems::seed_interactor_defaults, systems::apply_messages)
                    .chain()
                    .in_set(ObjectInteractionSystems::ReadCommands)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::refresh_candidates
                    .in_set(ObjectInteractionSystems::RefreshCandidates)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::acquire_selected_targets
                    .in_set(ObjectInteractionSystems::AcquireTargets)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.physics_schedule,
                physics::maintain_holds
                    .in_set(ObjectInteractionSystems::MaintainHold)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.physics_schedule,
                physics::release_and_throw
                    .in_set(ObjectInteractionSystems::ReleaseAndThrow)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                debug::refresh_diagnostics
                    .in_set(ObjectInteractionSystems::Presentation)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                PostUpdate,
                debug::draw_debug
                    .run_if(systems::runtime_is_active)
                    .run_if(debug::debug_gizmos_enabled)
                    .run_if(resource_exists::<GizmoConfigStore>),
            );

        if self.physics_schedule == PhysicsSchedule.intern() {
            app.configure_sets(
                PhysicsSchedule,
                (
                    ObjectInteractionSystems::MaintainHold,
                    ObjectInteractionSystems::ReleaseAndThrow,
                )
                    .chain()
                    .before(PhysicsStepSystems::First),
            );
        } else {
            app.configure_sets(
                self.physics_schedule,
                (
                    ObjectInteractionSystems::MaintainHold,
                    ObjectInteractionSystems::ReleaseAndThrow,
                )
                    .chain(),
            );
        }
    }
}

#[cfg(test)]
#[path = "plugin_tests.rs"]
mod plugin_tests;
