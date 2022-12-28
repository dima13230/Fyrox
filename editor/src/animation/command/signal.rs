use crate::{
    animation::command::fetch_animation_player,
    command::Command,
    define_universal_commands,
    scene::commands::{SceneCommand, SceneContext},
};
use fyrox::{
    animation::Animation,
    core::{pool::Handle, reflect::ResolvePath, uuid::Uuid},
    scene::node::Node,
};

define_universal_commands!(
    make_animation_signal_property_command,
    Command,
    SceneCommand,
    SceneContext,
    Uuid,
    ctx,
    handle,
    self,
    {
        fetch_animation_player(self.node_handle, ctx).animations_mut()[self.animation_handle]
            .signals_mut()
            .iter_mut()
            .find(|s| s.id == self.handle)
            .unwrap()
    },
    node_handle: Handle<Node>,
    animation_handle: Handle<Animation>
);
