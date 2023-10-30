#![feature(unboxed_closures, fn_traits)]

use std::{sync::{Arc, mpsc::{self, Receiver}}, borrow::Cow};

use model::{geom::{tri::TriMeshGeom, MeshAlloc}, AffineTransform, camera::{Camera, PerspectiveCamera}};
use na::{Matrix3, Vector3, UnitQuaternion};
use winit::{
    event_loop::{EventLoop, ControlFlow},
    window::{WindowBuilder, CursorGrabMode, Window},
    event::{Event, WindowEvent, DeviceEvent},
    keyboard::{PhysicalKey, KeyCode},
};

use totality_render::{Renderer, RendererPreferences, task::{RenderTask, DrawTask}};

pub enum WindowPurpose {
    Primary,
}

pub enum WorldEvent {
    SetMoveForward(bool),
    SetMoveBackward(bool),
    SetMoveLeft(bool),
    SetMoveRight(bool),
    SetMoveUp(bool),
    SetMoveDown(bool),

    SetRollLeft(bool),
    SetRollRight(bool),
    Yaw(f32),
    Pitch(f32),

    ShiftBackground,
}

pub struct StateMap {
    pub x_pos: bool,
    pub x_neg: bool,
    pub y_pos: bool,
    pub y_neg: bool,
    pub z_pos: bool,
    pub z_neg: bool,
    pub roll_left: bool,
    pub roll_right: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MouseMotionMode {
    Warp,
    Relative,
}
const FORCE_MOUSE_MOTION_MODE: Option<MouseMotionMode> = Some(MouseMotionMode::Warp);

pub struct RenderThread<'a> {
    window: Arc<Window>,
    input_rx: Receiver<WorldEvent>,
    state_map: StateMap,

    clear_color_mode: usize,
    base_clear_color: [f32; 4],

    camera: Camera,
    draw_tasks: Vec<DrawTask<'a>>,

    renderer: Renderer,
}

impl<'a> RenderThread<'a> {
    fn new(input_rx: Receiver<WorldEvent>, window: &Arc<Window>) -> Self {
        let preferences = RendererPreferences::default();
        let renderer = Renderer::init(Some("totality-render-demo".to_owned()), None, &*window, &preferences).unwrap();

        let camera = Camera::Perspective(PerspectiveCamera::default());

        let mut alloc = MeshAlloc::new();
        // Load up! This one's a simple triangle.
        let triangle_mesh = TriMeshGeom::triangle(
            &mut alloc,
            Matrix3::new(
                0.0, 0.5, 0.0,
                0.5, 0.0, 0.0,
                0.0, 0.0, 0.0,
            ),
            [[0., 0., 0.], [0., 0., 0.], [0., 0., 0.]],
            [[0.5, 0.], [0., 0.5], [0., 0.]],
            [0., 0., 0.],
            None,
        );
        let cube_mesh = Box::leak(Box::new(model::unit_cube(&mut alloc, None)));
        let base_clear_color = [0.5, 0.5, 0.5, 1.];

        let clear_color_mode = 0;
        let draw_tasks = vec![
            DrawTask {
                mesh: Cow::Owned(triangle_mesh.clone()),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos = Vector3::new(1., 0., 0.);
                    transform
                })],
            },
            DrawTask {
                mesh: Cow::Owned(triangle_mesh.clone()),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos = Vector3::new(-1., 0., 0.);
                    transform
                })],
            },
            DrawTask {
                mesh: Cow::Borrowed(cube_mesh),
                instancing_information: vec![
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(0.5, 0., 0.);
                        transform
                    }),
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(1.5, 0., 0.);
                        transform
                    }),
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(-0.5, 0., 0.);
                        transform
                    }),
                    Cow::Owned({
                        // x axis
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(1., 0., 0.);
                        transform.ori = UnitQuaternion::new(Vector3::z() * std::f32::consts::FRAC_PI_2);
                        transform.scaling.y = 0.2;
                        transform.scaling.z = 0.2;
                        transform
                    }),
                    Cow::Owned({
                        // y axis, this is the natural orientation
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(0., 1., 0.);
                        transform.scaling.x = 0.2;
                        transform.scaling.z = 0.2;
                        transform
                    }),
                    Cow::Owned({
                        // z axis
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(0., 0., 1.);
                        transform.ori = UnitQuaternion::new(Vector3::x() * std::f32::consts::FRAC_PI_2);
                        transform.scaling.x = 0.2;
                        transform.scaling.y = 0.2;
                        transform
                    }),
                ],
            },
        ];

        let state_map = StateMap {
            z_pos: false,
            z_neg: false,
            y_pos: false,
            y_neg: false,
            x_pos: false,
            x_neg: false,
            roll_right: false,
            roll_left: false,
        };

        Self {
            window: Arc::clone(window),
            input_rx,
            state_map,

            clear_color_mode,
            base_clear_color,

            camera,
            draw_tasks,

            renderer,
        }
    }
}

impl<'a> FnOnce<()> for RenderThread<'a> {
    type Output = ();

    extern "rust-call" fn call_once(mut self, _args: ()) -> Self::Output {
        'prime: loop {
            { // input handling
                let mut pitch_delta = 0.;
                let mut yaw_delta = 0.;
                while let Some(e) = match self.input_rx.try_recv() {
                    Ok(e) => Some(e),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break 'prime;
                    },
                } {
                    match e {
                        WorldEvent::SetMoveForward(state) => {
                            self.state_map.z_neg = state;
                        },
                        WorldEvent::SetMoveBackward(state) => {
                            self.state_map.z_pos = state;
                        },
                        WorldEvent::SetMoveLeft(state) => {
                            self.state_map.x_neg = state;
                        },
                        WorldEvent::SetMoveRight(state) => {
                            self.state_map.x_pos = state;
                        },
                        WorldEvent::SetMoveUp(state) => {
                            self.state_map.y_pos = state;
                        },
                        WorldEvent::SetMoveDown(state) => {
                            self.state_map.y_neg = state;
                        },
                        WorldEvent::SetRollLeft(state) => {
                            self.state_map.roll_left = state;
                        },
                        WorldEvent::SetRollRight(state) => {
                            self.state_map.roll_right = state;
                        },
                        WorldEvent::Yaw(delta) => {
                            yaw_delta += delta;
                        },
                        WorldEvent::Pitch(delta) => {
                            pitch_delta += delta;
                        },
                        WorldEvent::ShiftBackground => {
                            self.clear_color_mode = (self.clear_color_mode + 1) % 3;
                        },
                    }
                }
                let total_displacement = {
                    let mut displacement = Vector3::<f32>::zeros();
                    if self.state_map.z_pos {
                        displacement.z += 0.1;
                    }
                    if self.state_map.z_neg {
                        displacement.z -= 0.1;
                    }
                    if self.state_map.x_pos {
                        displacement.x += 0.1;
                    }
                    if self.state_map.x_neg {
                        displacement.x -= 0.1;
                    }
                    if self.state_map.y_pos {
                        displacement.y += 0.1;
                    }
                    if self.state_map.y_neg {
                        displacement.y -= 0.1;
                    }
                    displacement
                };
                let mut roll = 0.;
                let total_orientation = {
                    // Ideally we'd use a velocity of sorts instead of hard coding, but this is an
                    // example.
                    if self.state_map.roll_right {
                        roll += std::f32::consts::PI / 50.;
                    }
                    if self.state_map.roll_left {
                        roll -= std::f32::consts::PI / 50.;
                    }
                    // For the unit quaternion:
                    //   roll is about the x axis (and thus functions as pitch in our world space)
                    //   pitch is about the y axis (and thus functions as yaw in our world space)
                    //   yaw is about the z axis (and thus functions as roll in our world space)
                    UnitQuaternion::from_euler_angles(pitch_delta, yaw_delta, roll)
                };
                log::info!("CAMERA-SHIFT displacement={:?} rot_roll={roll} rot_pitch={pitch_delta} rot_yaw={yaw_delta}", total_displacement.as_slice());
                self.camera.trans_cam_space(total_displacement);
                self.camera.rot_cam_space(total_orientation);
            }

            let mut clear_color = self.base_clear_color.clone();
            clear_color[self.clear_color_mode] = 1.;

            self.renderer.render_to(Arc::clone(&self.window), RenderTask {
                cam: &self.camera,
                draws: self.draw_tasks.clone(),
                clear_color: self.base_clear_color.clone().into(),
            }).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

// Demo!
fn main() {
    // Setup logging
    simple_logger::init().unwrap();

    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().build(&event_loop).unwrap());

    // Setup communication mesh.
    let (tx, rx) = mpsc::channel::<WorldEvent>();

    std::thread::spawn(RenderThread::new(rx, &window));

    // We could *try* to seed this, but I'm lazy.
    let mut warp_mouse_detected = false;
    let mut last_mouse_x = None;
    let mut last_mouse_y = None;
    window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
    window.set_cursor_visible(false);
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(|event, elwt| {
        match event {
            Event::NewEvents(_cause) => {},
            Event::WindowEvent { window_id: _, event } => match event {
                WindowEvent::CloseRequested => { elwt.exit(); },
                _ => {},
            },
            Event::DeviceEvent { device_id: _device_id, event } => match event {
                DeviceEvent::Key(key_in) => match key_in.physical_key {
                    PhysicalKey::Code(keycode) => match keycode {
                        // We'll just ignore modifiers for now.
                        KeyCode::Escape => { elwt.exit(); },
                        KeyCode::KeyW => {
                            tx.send(WorldEvent::SetMoveForward(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyA => {
                            tx.send(WorldEvent::SetMoveLeft(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyS => {
                            tx.send(WorldEvent::SetMoveBackward(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyD => {
                            tx.send(WorldEvent::SetMoveRight(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyQ => {
                            tx.send(WorldEvent::SetRollLeft(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyE => {
                            tx.send(WorldEvent::SetRollRight(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::Space => {
                            tx.send(WorldEvent::SetMoveUp(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::ControlLeft => {
                            tx.send(WorldEvent::SetMoveDown(key_in.state.is_pressed())).unwrap();
                        },
                        _ => {},
                    },
                    PhysicalKey::Unidentified(_native) => {},
                },
                DeviceEvent::MouseMotion { delta: (maybe_xd, maybe_yd) } => {
                    let (xd, yd) = match FORCE_MOUSE_MOTION_MODE {
                        Some(MouseMotionMode::Relative) => (maybe_xd, maybe_yd),
                        Some(MouseMotionMode::Warp) => {
                            (calc_relative_motion(&mut last_mouse_x, maybe_xd), calc_relative_motion(&mut last_mouse_y, maybe_yd))
                        },
                        None => {
                            // We'll kind of guess if this is correct.
                            // Absolute values tend to be large -- break on > 2000.
                            // This can probably be better.
                            let is_probably_absolute = warp_mouse_detected || (maybe_xd * maybe_xd + maybe_yd * maybe_yd) > (2000. * 2000.);
                            if is_probably_absolute {
                                warp_mouse_detected = true;
                            }
                            if is_probably_absolute {
                                (calc_relative_motion(&mut last_mouse_x, maybe_xd), calc_relative_motion(&mut last_mouse_y, maybe_yd))
                            } else {
                                (maybe_xd, maybe_yd)
                            }
                        },
                    };

                    let scaling_factor = std::f64::consts::PI / 500.;
                    let x_scaling_factor = -scaling_factor;
                    let y_scaling_factor = -scaling_factor / 5.;
                    log::info!("MOUSE-MOVED x={xd} y={yd}");

                    tx.send(WorldEvent::Pitch((yd * y_scaling_factor) as f32)).unwrap();
                    tx.send(WorldEvent::Yaw((xd * x_scaling_factor) as f32)).unwrap();
                },
                _ => {},
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

fn calc_relative_motion(last: &mut Option<f64>, curr: f64) -> f64 {
    match last {
        None => {
            *last = Some(curr);
            0.
        },
        Some(ref mut last) => {
            let val = curr - *last;
            *last = curr;
            val
        },
    }
}
