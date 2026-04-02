use saddle_physics_object_interaction_example_common as common;
#[cfg(feature = "e2e")]
mod e2e;
#[cfg(feature = "e2e")]
mod scenarios;

use bevy::{
    prelude::*,
    remote::{RemotePlugin, http::RemoteHttpPlugin},
};
#[cfg(feature = "dev")]
use bevy_brp_extras::BrpExtrasPlugin;

fn main() {
    let mut app = App::new();
    common::configure_app(&mut app, common::DemoMode::Lab);
    app.add_plugins(RemotePlugin::default());
    #[cfg(feature = "dev")]
    app.add_plugins(BrpExtrasPlugin::with_http_plugin(
        RemoteHttpPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::ObjectInteractionLabE2EPlugin);
    app.run();
}
