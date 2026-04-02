use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    InteractionTarget, ObjectInteractionPlugin, ObjectInteractionState, ObjectReleased,
    components::{HeldBy, HeldRuntime, Holding, InteractionCandidates},
};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestActivate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestDeactivate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestUpdate;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestPhysics;

#[test]
fn plugin_registers_resources_and_messages() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_schedule(TestActivate);
    app.init_schedule(TestDeactivate);
    app.init_schedule(TestUpdate);
    app.init_schedule(TestPhysics);
    app.add_plugins(ObjectInteractionPlugin::new(
        TestActivate,
        TestDeactivate,
        TestUpdate,
        TestPhysics,
    ));

    assert!(
        app.world()
            .contains_resource::<crate::ObjectInteractionConfig>()
    );
    assert!(
        app.world()
            .contains_resource::<crate::ObjectInteractionDiagnostics>()
    );

    app.world_mut().write_message(crate::TryAcquireObject {
        interactor: Entity::PLACEHOLDER,
    });
}

#[test]
fn deactivate_schedule_releases_existing_hold() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_schedule(TestActivate);
    app.init_schedule(TestDeactivate);
    app.init_schedule(TestUpdate);
    app.init_schedule(TestPhysics);
    app.add_plugins(ObjectInteractionPlugin::new(
        TestActivate,
        TestDeactivate,
        TestUpdate,
        TestPhysics,
    ));

    let prop = app.world_mut().spawn_empty().id();
    let actor = app
        .world_mut()
        .spawn((
            Holding(prop),
            ObjectInteractionState::Holding(prop),
            InteractionTarget::default(),
            InteractionCandidates::default(),
        ))
        .id();
    app.world_mut().entity_mut(prop).insert((
        HeldBy(actor),
        HeldRuntime::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, None),
    ));

    app.world_mut().run_schedule(TestDeactivate);

    assert!(app.world().get::<Holding>(actor).is_none());
    assert!(app.world().get::<HeldBy>(prop).is_none());
    assert!(app.world().get::<HeldRuntime>(prop).is_none());

    let messages = app.world().resource::<Messages<ObjectReleased>>();
    let events: Vec<_> = messages.iter_current_update_messages().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].reason, crate::ReleaseReason::Deactivated);
    assert_eq!(events[0].object, prop);
}
