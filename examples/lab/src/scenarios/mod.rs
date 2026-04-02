use saddle_bevy_e2e::{action::Action, actions::assertions, scenario::Scenario};
use saddle_physics_object_interaction::{HeldBy, Holding};

use crate::common::{self, DemoDiagnostics, DemoStation, DemoWorld};

pub fn list_scenarios() -> Vec<&'static str> {
    vec![
        "smoke_launch",
        "object_interaction_smoke",
        "object_interaction_throw",
        "object_interaction_heavy_reject",
        "object_interaction_obstruction_break",
        "object_interaction_rotate_inspect",
    ]
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    match name {
        "smoke_launch" => Some(smoke_launch()),
        "object_interaction_smoke" => Some(object_interaction_smoke()),
        "object_interaction_throw" => Some(object_interaction_throw()),
        "object_interaction_heavy_reject" => Some(object_interaction_heavy_reject()),
        "object_interaction_obstruction_break" => Some(object_interaction_obstruction_break()),
        "object_interaction_rotate_inspect" => Some(object_interaction_rotate_inspect()),
        _ => None,
    }
}

fn station(station: DemoStation) -> Action {
    Action::Custom(Box::new(move |world| common::set_station(world, station)))
}

fn acquire() -> Action {
    Action::Custom(Box::new(common::send_try_acquire))
}

fn release() -> Action {
    Action::Custom(Box::new(common::send_release))
}

fn throw() -> Action {
    Action::Custom(Box::new(common::send_throw))
}

fn distance(delta: f32) -> Action {
    Action::Custom(Box::new(move |world| {
        common::send_adjust_distance(world, delta)
    }))
}

fn rotate_y(degrees: f32) -> Action {
    Action::Custom(Box::new(move |world| common::send_rotate_y(world, degrees)))
}

fn smoke_launch() -> Scenario {
    Scenario::builder("smoke_launch")
        .description("Boot the crate-local lab, settle the default crate-facing station, and capture the ready state.")
        .then(station(DemoStation::Crate))
        .then(Action::WaitFrames(12))
        .then(assertions::custom("crate station exposes a target", |world| {
            world
                .resource::<DemoDiagnostics>()
                .target_name
                .as_deref()
                == Some("Light Crate")
        }))
        .then(Action::Screenshot("smoke_ready".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("smoke_launch"))
        .build()
}

fn object_interaction_smoke() -> Scenario {
    Scenario::builder("object_interaction_smoke")
        .description("Acquire the default crate, hold it long enough for a clean screenshot, then drop it and verify the release reason.")
        .then(station(DemoStation::Crate))
        .then(Action::WaitFrames(10))
        .then(acquire())
        .then(Action::WaitFrames(24))
        .then(assertions::custom("crate is held", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            diagnostics.held_name.as_deref() == Some("Light Crate")
                && diagnostics.acquisition_count >= 1
                && world.get::<Holding>(demo.interactor).is_some()
                && world.get::<HeldBy>(demo.light_crate).is_some()
        }))
        .then(Action::Screenshot("held_crate".into()))
        .then(Action::WaitFrames(1))
        .then(release())
        .then(Action::WaitFrames(10))
        .then(assertions::custom("drop clears hold state", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            diagnostics.held_name.is_none()
                && diagnostics.last_released_reason.as_deref() == Some("Dropped")
                && world.get::<Holding>(demo.interactor).is_none()
                && world.get::<HeldBy>(demo.light_crate).is_none()
        }))
        .then(Action::Screenshot("crate_dropped".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("object_interaction_smoke"))
        .build()
}

fn object_interaction_throw() -> Scenario {
    Scenario::builder("object_interaction_throw")
        .description("Acquire the crate, throw it forward, and verify the throw/release messages plus the airborne visual state.")
        .then(station(DemoStation::Crate))
        .then(Action::WaitFrames(10))
        .then(acquire())
        .then(Action::WaitFrames(24))
        .then(Action::Screenshot("throw_ready".into()))
        .then(Action::WaitFrames(1))
        .then(throw())
        .then(Action::WaitFrames(10))
        .then(assertions::custom("throw increments counters and clears hold", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            let actor_z = world
                .get::<bevy::prelude::Transform>(demo.interactor)
                .map(|transform| transform.translation.z)
                .unwrap_or_default();
            let prop_z = world
                .get::<bevy::prelude::Transform>(demo.light_crate)
                .map(|transform| transform.translation.z)
                .unwrap_or_default();
            diagnostics.throw_count >= 1
                && diagnostics.held_name.is_none()
                && diagnostics.last_released_reason.as_deref() == Some("Thrown")
                && diagnostics.last_throw_impulse.z < -5.0
                && world.get::<Holding>(demo.interactor).is_none()
                && world.get::<HeldBy>(demo.light_crate).is_none()
                && prop_z < actor_z - 0.8
        }))
        .then(Action::Screenshot("crate_thrown".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("object_interaction_throw"))
        .build()
}

fn object_interaction_heavy_reject() -> Scenario {
    Scenario::builder("object_interaction_heavy_reject")
        .description("Aim at the overweight spool, attempt acquisition, and verify the failure reason without entering held state.")
        .then(station(DemoStation::Heavy))
        .then(Action::WaitFrames(10))
        .then(acquire())
        .then(Action::WaitFrames(10))
        .then(assertions::custom("heavy spool is rejected", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            diagnostics.held_name.is_none()
                && diagnostics.last_failure_reason.as_deref() == Some("TargetTooHeavy")
                && world.get::<Holding>(demo.interactor).is_none()
                && world.get::<HeldBy>(demo.heavy_spool).is_none()
        }))
        .then(Action::Screenshot("heavy_reject".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("object_interaction_heavy_reject"))
        .build()
}

fn object_interaction_obstruction_break() -> Scenario {
    Scenario::builder("object_interaction_obstruction_break")
        .description("Acquire the crate, move to the occlusion station, and verify that line-of-sight loss forces a release.")
        .then(station(DemoStation::Crate))
        .then(Action::WaitFrames(10))
        .then(acquire())
        .then(Action::WaitFrames(24))
        .then(Action::Screenshot("occlusion_before".into()))
        .then(Action::WaitFrames(1))
        .then(station(DemoStation::Occlusion))
        .then(Action::WaitFrames(30))
        .then(assertions::custom("occlusion forces release", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            diagnostics.held_name.is_none()
                && diagnostics.last_released_reason.as_deref() == Some("Occluded")
                && world.get::<Holding>(demo.interactor).is_none()
                && world.get::<HeldBy>(demo.light_crate).is_none()
        }))
        .then(Action::Screenshot("occlusion_after".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("object_interaction_obstruction_break"))
        .build()
}

fn object_interaction_rotate_inspect() -> Scenario {
    Scenario::builder("object_interaction_rotate_inspect")
        .description("Acquire the inspect prop, pull it closer, rotate it in place, and verify close inspection remains stable.")
        .then(station(DemoStation::Inspect))
        .then(Action::WaitFrames(10))
        .then(acquire())
        .then(Action::WaitFrames(22))
        .then(Action::Screenshot("inspect_ready".into()))
        .then(Action::WaitFrames(1))
        .then(distance(-0.6))
        .then(rotate_y(60.0))
        .then(Action::WaitFrames(18))
        .then(assertions::custom("inspect prism stays held nearby", |world| {
            let diagnostics = world.resource::<DemoDiagnostics>();
            let demo = world.resource::<DemoWorld>();
            diagnostics.held_name.as_deref() == Some("Inspect Prism")
                && diagnostics.hold_distance < 1.8
                && diagnostics.unstable_count == 0
                && world
                    .get::<Holding>(demo.interactor)
                    .is_some_and(|holding| holding.0 == demo.inspect_prism)
                && world
                    .get::<HeldBy>(demo.inspect_prism)
                    .is_some_and(|held_by| held_by.0 == demo.interactor)
        }))
        .then(Action::Screenshot("inspect_rotated".into()))
        .then(Action::WaitFrames(1))
        .then(assertions::log_summary("object_interaction_rotate_inspect"))
        .build()
}
