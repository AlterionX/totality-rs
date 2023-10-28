use std::{sync::Arc, borrow::Cow};

use model::{geom::tri::TriMeshGeom, AffineTransform, camera::{Camera, PerspectiveCamera}};
use na::Matrix3;
use vulkano::format::ClearColorValue;
use winit::{
    event_loop::{EventLoop, ControlFlow},
    window::WindowBuilder,
    event::{Event, WindowEvent, DeviceEvent},
    keyboard::{PhysicalKey, KeyCode},
};

use totality_render::{Renderer, RendererPreferences, task::RenderTask};

pub enum WindowPurpose {
    Primary,
}

// Demo!
fn main() {
    // Setup logging
    simple_logger::init().unwrap();

    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().build(&event_loop).unwrap());

    let preferences = RendererPreferences::default();
    let mut renderer = Renderer::init(Some("totality-render-demo".to_owned()), None, &*window, &preferences).unwrap();

    let win0 = Arc::clone(&window);
    std::thread::spawn(move || {
        let mut camera = Camera::Perspective(PerspectiveCamera::default());
        // Load up! This one's a simple triangle.
        let triangle_mesh = TriMeshGeom::triangle(
            Matrix3::new(
                0.5, 0.0, 0.0,
                0.0, 0.5, 0.0,
                0.0, 0.0, 0.0
            ),
            vec![[0., 0., 0.], [0., 0., 0.], [0., 0., 0.]],
            vec![[0.5, 0.], [0., 0.5], [0.5, 0.5]],
            [0., 0., 0.],
            None,
        );
        let cube_mesh = model::unit_cube(None);
        let base_clear_color = [0., 0., 0., 1.];

        let mut state = 0;
        renderer.load_model(triangle_mesh).unwrap();
        renderer.render_to(Arc::clone(&win0), RenderTask {
            cam: &camera,
            instancing_information: vec![Cow::Owned(AffineTransform::identity())],
            clear_color: base_clear_color.clone().into(),
        }).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // renderer.load_model(cube_mesh).unwrap();
        loop {
            state = (state + 1) % 3;
            let mut clear_color = base_clear_color.clone();
            clear_color[state] = 1.;

            renderer.render_to(Arc::clone(&win0), RenderTask {
                cam: &camera,
                instancing_information: vec![Cow::Owned(AffineTransform::identity())],
                clear_color: clear_color.into(),
            }).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });

    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(|event, elwt| {
        match event {
            Event::NewEvents(cause) => {},
            Event::WindowEvent { window_id: _, event } => match event {
                WindowEvent::CloseRequested => { elwt.exit(); },
                _ => {},
            },
            Event::DeviceEvent { device_id: _, event } => {
                match event {
                    DeviceEvent::Key(key_in) => match key_in.physical_key {
                        PhysicalKey::Code(keycode) => match keycode {
                            // We'll just ignore modifiers for now.
                            KeyCode::Escape => { elwt.exit(); },
                            _ => {},
                        },
                        PhysicalKey::Unidentified(_native) => {},
                    },
                    _ => {},
                }
            },
            Event::UserEvent(_ue) => {},
            Event::Resumed => {},
            Event::Suspended => {},
            Event::AboutToWait => {},
            Event::LoopExiting => {},
            Event::MemoryWarning => {},
        }
    }).unwrap();
}
