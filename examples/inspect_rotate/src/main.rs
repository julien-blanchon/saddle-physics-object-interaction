use saddle_physics_object_interaction_example_common as common;

fn main() {
    let mut app = bevy::prelude::App::new();
    common::configure_app(&mut app, common::DemoMode::InspectRotate);
    app.run();
}
