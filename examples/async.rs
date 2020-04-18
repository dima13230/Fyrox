//! Example 02. Asynchronous scene loading.
//!
//! Difficulty: Medium.
//!
//! This example shows how to load scene in separate thread and how create standard
//! loading screen which will show progress.

extern crate rg3d;

use std::{
    time::Instant,
    sync::{Arc, Mutex},
};
use rg3d::{
    scene::{
        base::BaseBuilder,
        transform::TransformBuilder,
        camera::CameraBuilder,
        node::Node,
        Scene,
    },
    engine::resource_manager::ResourceManager,
    gui::{
        widget::WidgetBuilder,
        text::TextBuilder,
        node::StubNode,
        progress_bar::ProgressBarBuilder,
        grid::{GridBuilder, Row, Column},
        VerticalAlignment,
        Thickness,
        HorizontalAlignment,
    },
    event::{
        Event,
        WindowEvent,
        DeviceEvent,
        VirtualKeyCode,
        ElementState,
    },
    event_loop::{
        EventLoop,
        ControlFlow,
    },
    core::{
        color::Color,
        pool::Handle,
        math::{
            vec3::Vec3,
            quat::Quat,
            vec2::Vec2,
        },
    },
    animation::Animation,
    utils::translate_event,
};

// Create our own engine type aliases. These specializations are needed
// because engine provides a way to extend UI with custom nodes and messages.
type GameEngine = rg3d::engine::Engine<(), StubNode>;
type UserInterface = rg3d::gui::UserInterface<(), StubNode>;
type UiNode = rg3d::gui::node::UINode<(), StubNode>;

struct Interface {
    root: Handle<UiNode>,
    debug_text: Handle<UiNode>,
    progress_bar: Handle<UiNode>,
    progress_text: Handle<UiNode>,
}

fn create_ui(ui: &mut UserInterface, screen_size: Vec2) -> Interface {
    let debug_text;
    let progress_bar;
    let progress_text;
    let root = GridBuilder::new(WidgetBuilder::new()
        .with_width(screen_size.x)
        .with_height(screen_size.y)
        .with_child({
            debug_text = TextBuilder::new(WidgetBuilder::new()
                .on_row(0)
                .on_column(0))
                .build(ui);
            debug_text
        })
        .with_child({
            progress_bar = ProgressBarBuilder::new(WidgetBuilder::new()
                .on_row(1)
                .on_column(1))
                .build(ui);
            progress_bar
        })
        .with_child({
            progress_text = TextBuilder::new(WidgetBuilder::new()
                .on_column(1)
                .on_row(0)
                .with_margin(Thickness::bottom(20.0))
                .with_vertical_alignment(VerticalAlignment::Bottom))
                .with_horizontal_text_alignment(HorizontalAlignment::Center)
                .build(ui);
            progress_text
        }))
        .add_row(Row::stretch())
        .add_row(Row::strict(30.0))
        .add_row(Row::stretch())
        .add_column(Column::stretch())
        .add_column(Column::strict(200.0))
        .add_column(Column::stretch())
        .build(ui);

    Interface {
        root,
        debug_text,
        progress_bar,
        progress_text,
    }
}

struct GameScene {
    scene: Scene,
    model_handle: Handle<Node>,
    walk_animation: Handle<Animation>,
}

struct SceneLoadContext {
    data: Option<GameScene>,
    message: String,
    progress: f32,
}

impl SceneLoadContext {
    pub fn report_progress(&mut self, progress: f32, message: &str) {
        self.progress = progress;
        self.message = message.to_owned();
        println!("Loading progress: {}% - {}", progress * 100.0, message);
    }
}

fn create_scene_async(resource_manager: Arc<Mutex<ResourceManager>>) -> Arc<Mutex<SceneLoadContext>> {
    // Create load context - it will be shared with caller and loader threads.
    let context = Arc::new(Mutex::new(SceneLoadContext {
        data: None,
        message: "Starting..".to_string(),
        progress: 0.0,
    }));
    let result = context.clone();

    // Spawn separate thread which will create scene by loading various assets.
    std::thread::spawn(move || {
        let mut scene = Scene::new();

        let mut resource_manager = resource_manager.lock().unwrap();

        // It is important to lock context for short period of time so other thread can
        // read data from it as soon as possible - not when everything was loaded.
        context.lock().unwrap().report_progress(0.0, "Creating camera...");

        // Camera is our eyes in the world - you won't see anything without it.
        let camera = CameraBuilder::new(BaseBuilder::new()
            .with_local_transform(TransformBuilder::new()
                .with_local_position(Vec3::new(0.0, 6.0, -12.0))
                .build()))
            .build();

        scene.graph.add_node(Node::Camera(camera));

        context.lock().unwrap().report_progress(0.33, "Loading model...");

        // Load model resource. Is does *not* adds anything to our scene - it just loads a
        // resource then can be used later on to instantiate models from it on scene. Why
        // loading of resource is separated from instantiation? Because there it is too
        // inefficient to load a resource every time you trying to create instance of it -
        // much more efficient is to load it one and then make copies of it. In case of
        // models it is very efficient because single vertex and index buffer can be used
        // for all models instances, so memory footprint on GPU will be lower.
        let model_resource = resource_manager.request_model("examples/data/mutant.FBX").unwrap();

        // Instantiate model on scene - but only geometry, without any animations.
        // Instantiation is a process of embedding model resource data in desired scene.
        let model_handle = model_resource.lock()
            .unwrap()
            .instantiate_geometry(&mut scene);

        // Now we have whole sub-graph instantiated, we can start modifying model instance.
        scene.graph[model_handle]
            .local_transform_mut()
            // Our model is too big, fix it by scale.
            .set_scale(Vec3::new(0.05, 0.05, 0.05));

        context.lock().unwrap().report_progress(0.66, "Loading animation...");

        // Add simple animation for our model. Animations are loaded from model resources -
        // this is because animation is a set of skeleton bones with their own transforms.
        let walk_animation_resource = resource_manager.request_model("examples/data/walk.fbx").unwrap();

        // Once animation resource is loaded it must be re-targeted to our model instance.
        // Why? Because animation in *resource* uses information about *resource* bones,
        // not model instance bones, retarget_animations maps animations of each bone on
        // model instance so animation will know about nodes it should operate on.
        let walk_animation = *walk_animation_resource
            .lock()
            .unwrap()
            .retarget_animations(model_handle, &mut scene)
            .get(0)
            .unwrap();

        context.lock().unwrap().report_progress(1.0, "Done");

        context.lock().unwrap().data = Some(GameScene {
            scene,
            model_handle,
            walk_animation,
        })
    });

    // Immediately return shared context.
    result
}

struct InputController {
    rotate_left: bool,
    rotate_right: bool,
}

fn main() {
    let event_loop = EventLoop::new();

    let window_builder = rg3d::window::WindowBuilder::new()
        .with_title("Example - Asynchronous Scene Loading")
        .with_resizable(true);

    let mut engine = GameEngine::new(window_builder, &event_loop).unwrap();

    // Prepare resource manager - it must be notified where to search textures. When engine
    // loads model resource it automatically tries to load textures it uses. But since most
    // model formats store absolute paths, we can't use them as direct path to load texture
    // instead we telling engine to search textures in given folder.
    engine.resource_manager.lock().unwrap().set_textures_path("examples/data");

    // Create simple user interface that will show some useful info.
    let window = engine.get_window();
    let screen_size = window.inner_size().to_logical(window.scale_factor());
    let interface = create_ui(&mut engine.user_interface, Vec2::new(screen_size.width, screen_size.height));

    // Create scene asynchronously - this method immediately returns empty load context
    // which will be filled with data over time.
    let game_scene = create_scene_async(engine.resource_manager.clone());

    // Initially these handles are None, once scene is loaded they'll be assigned.
    let mut scene_handle = Handle::NONE;
    let mut model_handle = Handle::NONE;
    let mut walk_animation = Handle::NONE;

    // Set ambient light.
    engine.renderer.set_ambient_color(Color::opaque(200, 200, 200));

    let clock = Instant::now();
    let fixed_timestep = 1.0 / 60.0;
    let mut elapsed_time = 0.0;

    // We will rotate model using keyboard input.
    let mut model_angle = 180.0f32.to_radians();

    // Create input controller - it will hold information about needed actions.
    let mut input_controller = InputController { rotate_left: false, rotate_right: false };

    // Finally run our event loop which will respond to OS and window events and update
    // engine state accordingly. Engine lets you to decide which event should be handled,
    // this is minimal working example if how it should be.
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::MainEventsCleared => {
                // This main game loop - it has fixed time step which means that game
                // code will run at fixed speed even if renderer can't give you desired
                // 60 fps.
                let mut dt = clock.elapsed().as_secs_f32() - elapsed_time;
                while dt >= fixed_timestep {
                    dt -= fixed_timestep;
                    elapsed_time += fixed_timestep;

                    // ************************
                    // Put your game logic here.
                    // ************************

                    // Check each frame if our scene is created - here we just trying to lock context
                    // without blocking, it is important for main thread to be functional while other
                    // thread still loading data.
                    if let Ok(mut load_context) = game_scene.try_lock() {
                        if let Some(game_scene) = load_context.data.take() {
                            // Add scene to engine - engine will take ownership over scene and will return
                            // you a handle to scene which can be used later on to borrow it and do some
                            // actions you need.
                            scene_handle = engine.scenes.add(game_scene.scene);
                            model_handle = game_scene.model_handle;
                            walk_animation = game_scene.walk_animation;

                            // Once scene is loaded, we should hide progress bar and text.
                            if let UiNode::ProgressBar(progress_bar) = engine.user_interface.node_mut(interface.progress_bar) {
                                progress_bar.set_visibility(false);
                            }

                            if let UiNode::Text(progress_text) = engine.user_interface.node_mut(interface.progress_text) {
                                progress_text.set_visibility(false);
                            }
                        }

                        // Report progress in UI.
                        if let UiNode::ProgressBar(progress_bar) = engine.user_interface.node_mut(interface.progress_bar) {
                            progress_bar.set_progress(load_context.progress);
                        }

                        if let UiNode::Text(progress_text) = engine.user_interface.node_mut(interface.progress_text) {
                            progress_text.set_text(format!("Loading scene: {}%\n{}", load_context.progress * 100.0, load_context.message));
                        }
                    }

                    // Update scene only if it is loaded.
                    if scene_handle.is_some() {
                        // Use stored scene handle to borrow a mutable reference of scene in
                        // engine.
                        let scene = &mut engine.scenes[scene_handle];

                        // Our animation must be applied to scene explicitly, otherwise
                        // it will have no effect.
                        scene.animations
                            .get_mut(walk_animation)
                            .get_pose()
                            .apply(&mut scene.graph);

                        // Rotate model according to input controller state.
                        if input_controller.rotate_left {
                            model_angle -= 5.0f32.to_radians();
                        } else if input_controller.rotate_right {
                            model_angle += 5.0f32.to_radians();
                        }

                        scene.graph[model_handle]
                            .local_transform_mut()
                            .set_rotation(Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), model_angle));
                    }

                    // While scene is loading, we will update progress bar.
                    if let UiNode::Text(text) = engine.user_interface.node_mut(interface.debug_text) {
                        let fps = engine.renderer.get_statistics().frames_per_second;
                        text.set_text(format!("Example 02 - Asynchronous Scene Loading\nUse [A][D] keys to rotate model.\nFPS: {}", fps));
                    }

                    // It is very important to "pump" messages from UI. Even if don't need to
                    // respond to such message, you should call this method, otherwise UI
                    // might behave very weird.
                    while let Some(_ui_event) = engine.user_interface.poll_message() {
                        // ************************
                        // Put your data model synchronization code here. It should
                        // take message and update data in your game according to
                        // changes in UI.
                        // ************************
                    }

                    engine.update(fixed_timestep);
                }

                // Rendering must be explicitly requested and handled after RedrawRequested event is received.
                engine.get_window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Run renderer at max speed - it is not tied to game code.
                engine.render(fixed_timestep).unwrap();
            }
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::Resized(size) => {
                        // It is very important to handle Resized event from window, because
                        // renderer knows nothing about window size - it must be notified
                        // directly when window size has changed.
                        engine.renderer.set_frame_size(size.into());

                        // Root UI node should be resized too, otherwise progress bar will stay
                        // in wrong position after resize.
                        let size = size.to_logical(engine.get_window().scale_factor());
                        if let UiNode::Grid(root) = engine.user_interface.node_mut(interface.root) {
                            root.set_width_mut(size.width)
                                .set_height_mut(size.height);
                        }
                    }
                    _ => ()
                }

                // It is very important to "feed" user interface (UI) with events coming
                // from main window, otherwise UI won't respond to mouse, keyboard, or any
                // other event.
                if let Some(os_event) = translate_event(&event) {
                    engine.user_interface.process_os_event(&os_event);
                }
            }
            Event::DeviceEvent { event, .. } => {
                if let DeviceEvent::Key(key) = event {
                    if let Some(key_code) = key.virtual_keycode {
                        match key_code {
                            VirtualKeyCode::A => input_controller.rotate_left = key.state == ElementState::Pressed,
                            VirtualKeyCode::D => input_controller.rotate_right = key.state == ElementState::Pressed,
                            _ => ()
                        }
                    }
                }
            }
            _ => *control_flow = ControlFlow::Poll,
        }
    });
}