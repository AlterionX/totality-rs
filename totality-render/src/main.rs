#![feature(unboxed_closures, fn_traits)]

use std::{sync::{Arc, mpsc::{self, Receiver}, Mutex}, borrow::Cow, time::Instant};

use model::{geom::{tri::TriMeshGeom, MeshAlloc}, AffineTransform, camera::{Camera, PerspectiveCamera}};
use na::{Matrix3, Vector3, UnitQuaternion, UnitVector3, Vector4};
use winit::{
    event_loop::{EventLoop, ControlFlow},
    window::{WindowBuilder, CursorGrabMode, Window},
    event::{Event, WindowEvent, DeviceEvent},
    keyboard::{PhysicalKey, KeyCode},
};

use totality_render::{Renderer, RendererPreferences, task::{RenderTask, DrawTask, LightCollection, Light, PointLight, DirectionalLight}};

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

    ShiftBackground(bool),
    ToggleWireFrame(bool),
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
    pub wireframe: bool,
    pub shift_background: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MouseMotionMode {
    Warp,
    Relative,
}
const FORCE_MOUSE_MOTION_MODE: Option<MouseMotionMode> = Some(MouseMotionMode::Warp);

#[derive(Debug, Clone)]
pub struct SimState {
    pub camera: Camera,
    pub is_wireframe: bool,
    pub clear_color_mode: usize,
    pub light_direction: UnitVector3<f32>,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            camera: Camera::Perspective(PerspectiveCamera::default()),
            is_wireframe: false,
            // 3 is the black state -- we'll start there.
            clear_color_mode: 3,
            light_direction: UnitVector3::new_normalize(Vector3::new(1., 1., 1.)),
        }
    }
}

pub struct SimThread {
    input_rx: Receiver<WorldEvent>,
    state_map: StateMap,

    // TODO tribuffer here
    sim_state: Arc<Mutex<SimState>>,

    previous_iteration: Option<Instant>,
}

impl SimThread {
    fn new(input_rx: Receiver<WorldEvent>, sim_state: Arc<Mutex<SimState>>) -> Self {
        let state_map = StateMap {
            z_pos: false,
            z_neg: false,
            y_pos: false,
            y_neg: false,
            x_pos: false,
            x_neg: false,
            roll_right: false,
            roll_left: false,
            shift_background: false,
            wireframe: false,
        };

        Self {
            input_rx,
            state_map,

            sim_state,

            previous_iteration: None,
        }
    }
}

impl FnOnce<()> for SimThread {
    type Output = ();

    extern "rust-call" fn call_once(mut self, _args: ()) -> Self::Output {
        let mut prior_log = Instant::now();
        'prime: loop {
            let iteration_start = Instant::now();
            let Some(prior_iteration) = self.previous_iteration else {
                self.previous_iteration = Some(iteration_start);
                continue;
            };
            let stashed_elapsed_time = iteration_start - prior_iteration;
            let elapsed = (stashed_elapsed_time.as_nanos() as f32) / 1000000.;
            self.previous_iteration = Some(iteration_start);

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
                    WorldEvent::ToggleWireFrame(state) => {
                        // Detect the upwards edge only.
                        if state && !self.state_map.wireframe {
                            let mut sim = self.sim_state.lock().unwrap();
                            sim.is_wireframe = !sim.is_wireframe;
                        }
                        self.state_map.wireframe = state;
                    },
                    WorldEvent::ShiftBackground(state) => {
                        // Detect the upwards edge only.
                        if state && !self.state_map.shift_background {
                            let mut sim = self.sim_state.lock().unwrap();
                            sim.clear_color_mode = (sim.clear_color_mode + 1) % 4;
                        }
                        self.state_map.shift_background = state;
                    },
                }
            }
            let timed_displacement = {
                let mut displacement = Vector3::<f32>::zeros();
                if self.state_map.z_pos {
                    displacement.z += 0.005;
                }
                if self.state_map.z_neg {
                    displacement.z -= 0.005;
                }
                if self.state_map.x_pos {
                    displacement.x += 0.005;
                }
                if self.state_map.x_neg {
                    displacement.x -= 0.005;
                }
                if self.state_map.y_pos {
                    displacement.y += 0.005;
                }
                if self.state_map.y_neg {
                    displacement.y -= 0.005;
                }
                displacement
            };
            let mut timed_roll = 0.;
            // Ideally we'd use a velocity of sorts instead of hard coding, but this is an
            // example.
            if self.state_map.roll_right {
                timed_roll += std::f32::consts::PI / 5000.;
            }
            if self.state_map.roll_left {
                timed_roll -= std::f32::consts::PI / 5000.;
            }

            // Print every once in a while to cut down logs.
            if (Instant::now() - prior_log).as_secs() > 1 {
                log::info!("CAMERA-SHIFT displacement={:?} rot_roll={timed_roll} rot_pitch={pitch_delta} rot_yaw={yaw_delta}", timed_displacement.as_slice());
                prior_log = Instant::now();
            }

            // For the unit quaternion:
            //   roll is about the x axis (and thus functions as pitch in our world space)
            //   pitch is about the y axis (and thus functions as yaw in our world space)
            //   yaw is about the z axis (and thus functions as roll in our world space)
            let total_rotation = UnitQuaternion::from_euler_angles(pitch_delta, yaw_delta, timed_roll * elapsed);
            let total_displacement = timed_displacement * elapsed;

            let mut sim = self.sim_state.lock().unwrap();
            sim.camera.trans_cam_space(total_displacement);
            sim.camera.rot_cam_space(total_rotation);

            let ori = UnitQuaternion::new(Vector3::x() * std::f32::consts::PI / 1000. * elapsed).to_homogeneous();
            let original_direction = Vector4::new(sim.light_direction.x, sim.light_direction.y, sim.light_direction.z, 1.);
            sim.light_direction = UnitVector3::new_normalize((ori * original_direction).xyz());
        }
    }
}

pub struct RenderThread<'a> {
    window: Arc<Window>,

    base_clear_color: [f32; 4],

    sim_state: Arc<Mutex<SimState>>,
    draw_tasks: Vec<DrawTask<'a>>,

    renderer: Renderer,
}

impl<'a> RenderThread<'a> {
    fn new(sim_state: Arc<Mutex<SimState>>, window: &Arc<Window>) -> Self {
        let preferences = RendererPreferences::default();
        let renderer = Renderer::init(Some("totality-render-demo".to_owned()), None, Arc::clone(window), &preferences).unwrap();

        let mut alloc = MeshAlloc::new();
        // Load up! This one's a simple triangle.
        let triangle_mesh = Box::leak(Box::new(TriMeshGeom::triangle(
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
        )));
        let cube_mesh = Box::leak(Box::new(model::unit_cube(&mut alloc, None)));
        let textured_cube_mesh = Box::leak(Box::new(model::unit_cube(&mut alloc, Some("../resources/logo.png".to_owned()))));
        let base_clear_color = [0.5, 0.5, 0.5, 1.];

        // 4 denotes "black", we'll just start there.
        let draw_tasks = vec![
            DrawTask {
                mesh: Cow::Borrowed(triangle_mesh),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos = Vector3::new(1., 0., 0.);
                    transform
                })],
            },
            DrawTask {
                mesh: Cow::Borrowed(triangle_mesh),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos = Vector3::new(-1., 0., 0.);
                    transform.scaling.x = 1000.;
                    transform
                })],
            },
            DrawTask {
                mesh: Cow::Borrowed(cube_mesh),
                instancing_information: vec![
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(0.7, 0., 1.);
                        transform
                    }),
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(1.7, 0., 1.);
                        transform
                    }),
                    Cow::Owned({
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(2.7, 0., 1.);
                        transform
                    }),
                    Cow::Owned({
                        // x axis
                        let mut transform = AffineTransform::identity();
                        transform.pos += Vector3::new(1., 0., 0.);
                        transform.ori = UnitQuaternion::new(Vector3::y() * std::f32::consts::FRAC_PI_2);
                        transform.scaling.x = 0.2;
                        transform.scaling.y = 0.2;
                        transform
                    }),
                    Cow::Owned({
                        // y axis, this is the "natural" orientation
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
                        transform.scaling.x = 0.2;
                        transform.scaling.y = 0.2;
                        transform
                    }),
                ],
            },
            DrawTask {
                mesh: Cow::Borrowed(textured_cube_mesh),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos += Vector3::new(1., 2., 1.);
                    transform
                })],
            },
            DrawTask {
                mesh: Cow::Owned(model::plane(&mut alloc, None)),
                instancing_information: vec![Cow::Owned({
                    let mut transform = AffineTransform::identity();
                    transform.pos += Vector3::new(0., -3., 0.);
                    transform
                })],
            },
        ];

        Self {
            window: Arc::clone(window),

            base_clear_color,

            sim_state,
            draw_tasks,

            renderer,
        }
    }
}

impl<'a> FnOnce<()> for RenderThread<'a> {
    type Output = ();

    extern "rust-call" fn call_once(mut self, _args: ()) -> Self::Output {
        loop {
            let sim = self.sim_state.lock().unwrap().clone();
            let clear_color = if sim.clear_color_mode == 3 {
                [0., 0., 0., 1.]
            } else {
                let mut cc = self.base_clear_color.clone();
                cc[sim.clear_color_mode] = 1.;
                cc
            };

            log::info!("CAMERA {:?}", sim.camera);
            self.renderer.render_to(Arc::clone(&self.window), RenderTask {
                draw_wireframe: sim.is_wireframe,
                cam: &sim.camera.clone(),
                draws: self.draw_tasks.clone(),
                clear_color: clear_color.into(),
                lights: LightCollection(vec![Light::Directional(DirectionalLight {
                    color: Vector3::new(1., 1., 1.),
                    direction: sim.light_direction,
                })]),
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
    let sim_state = Arc::new(Mutex::new(SimState::default()));

    std::thread::spawn(SimThread::new(rx, Arc::clone(&sim_state)));
    std::thread::spawn(RenderThread::new(Arc::clone(&sim_state), &window));

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
                            tx.send(WorldEvent::SetRollRight(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyE => {
                            tx.send(WorldEvent::SetRollLeft(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::Space => {
                            tx.send(WorldEvent::SetMoveUp(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::ControlLeft => {
                            tx.send(WorldEvent::SetMoveDown(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::KeyN => {
                            tx.send(WorldEvent::ShiftBackground(key_in.state.is_pressed())).unwrap();
                        },
                        KeyCode::Tab => {
                            tx.send(WorldEvent::ToggleWireFrame(key_in.state.is_pressed())).unwrap();
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
