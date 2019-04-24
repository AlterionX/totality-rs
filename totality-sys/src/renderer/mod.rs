#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx11",
        feature = "dx12",
        feature = "metal",
        feature = "gl"
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

extern crate gfx_hal as hal;

mod shaders;
mod buffers;
mod texture;

use self::hal::{
    *,
    format::*,
    window::*,
    image::*,
    pass::*,
    pool::*,
    command::*,
    pso::*,
};
use super::av::ArrayVec;
use std::{
    ops::DerefMut,
    time::SystemTime,
    sync::{Arc, Mutex, RwLock, mpsc::{Sender, Receiver, channel, SendError}},
    mem::{size_of, ManuallyDrop},
    ptr::read,
    marker::PhantomData,
};
use super::na::Vector4;
use winit::Window;
use super::kt::KillableThread;
use super::geom::{self, scene::Scene};
use buffers::{AllocatedBuffer, LoadedBuffer};
use shaders::{ShaderInfo, CompiledShader};

#[allow(dead_code)]
use log::{error, warn, info, debug, trace};

const VERTEX_SOURCE: &str = include_str!("../../resources/shaders/basic.vert");
const FRAGMENT_SOURCE: &str =  include_str!("../../resources/shaders/basic.frag");

pub trait RendererCreator<I: Instance>: Fn(&Window) -> Result<Renderer<I>, &'static str> + Send + 'static {}
impl <F: Fn(&Window) -> Result<Renderer<I>, &'static str> + Send + 'static, I: Instance> RendererCreator<I> for F {}
pub trait RenderFn<I: Instance>: FnMut(&mut Renderer<I>) + Send + 'static {}
impl <F: FnMut(&mut Renderer<I>) + Send + 'static, I: Instance> RenderFn<I> for F {}
pub struct Color(pub Vector4<f32>);
pub enum RenderReq<I: Instance> {
    Restart,
    Clear(Color),
    Seq(Vec<RenderReq<I>>),
    Draw(geom::Model, Color),
    Free(Arc<Mutex<RenderFn<I>>>),
}

pub struct Renderer<I: Instance> {
    current_frame: usize,
    max_frames_in_flight: usize,

    in_flight_fences: Vec<<<I>::Backend as Backend>::Fence>,
    render_finished_semaphores: Vec<<<I>::Backend as Backend>::Semaphore>,
    image_available_semaphores: Vec<<<I>::Backend as Backend>::Semaphore>,

    command_buffers: Vec<CommandBuffer<<I>::Backend, Graphics, MultiShot, Primary>>,
    command_pool: ManuallyDrop<CommandPool<<I>::Backend, Graphics>>,

    framebuffers: Vec<<<I>::Backend as Backend>::Framebuffer>,
    image_views: Vec<(<<I>::Backend as Backend>::ImageView)>,

    alloc_buffers: Vec<AllocatedBuffer<<I>::Backend>>,
    loaded_buffers: Vec<LoadedBuffer<Box<geom::Geom>, <I>::Backend>>,

    graphics_pipeline: ManuallyDrop<<<I>::Backend as Backend>::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<<<I>::Backend as Backend>::PipelineLayout>,
    descriptor_set_layouts: Vec<<<I>::Backend as Backend>::DescriptorSetLayout>,

    render_pass: ManuallyDrop<<<I>::Backend as Backend>::RenderPass>,
    render_area: Rect,
    queue_group: ManuallyDrop<QueueGroup<<I>::Backend, Graphics>>,
    swapchain: ManuallyDrop<<<I>::Backend as Backend>::Swapchain>,

    device: ManuallyDrop<<<I>::Backend as Backend>::Device>,
    _adapter: Adapter<<I>::Backend>,
    _surface: <<I>::Backend as Backend>::Surface,
    _instance: ManuallyDrop<I>,
}
impl<I: Instance> Renderer<I> {
    fn new(w: &Window, inst: I, mut surf: <<I>::Backend as Backend>::Surface) -> Result<Self, &'static str> {
        let adapter = hal::Instance::enumerate_adapters(&inst)
            .into_iter()
            .find(|a| {
                a.queue_families.iter()
                    .any(|qf| qf.supports_graphics() && surf.supports_queue_family(qf))
             })
             .ok_or("Couldn't find a graphical Adapter!")?;
        let (mut device, queue_group) = {
            let queue_family = adapter
                .queue_families
                .iter()
                .find(|qf| qf.supports_graphics() && surf.supports_queue_family(qf))
                .ok_or("Couldn't find a QueueFamily with graphics!")?;
            let Gpu { device, mut queues } = unsafe {
                adapter.physical_device
                    .open(&[(&queue_family, &[1.0; 1])])
                    .map_err(|_| "Couldn't open the PhysicalDevice!")?
            };
            let queue_group = queues
                .take::<Graphics>(queue_family.id())
                .ok_or("Couldn't take ownership of the QueueGroup!")?;
            let _ = if queue_group.queues.len() > 0 {
                Ok(())
            } else {
                Err("The QueueGroup did not have any CommandQueues available!")
            }?;
            (device, queue_group)
        };
        let (swapchain, extent, backbuffer, format, max_frames_in_flight) = {
            let (caps, preferred_formats, present_modes, composite_alphas) = surf.compatibility(&adapter.physical_device);
            info!("{:?}", caps);
            info!("Preferred Formats: {:?}", preferred_formats);
            info!("Present Modes: {:?}", present_modes);
            info!("Composite Alphas: {:?}", composite_alphas);
            //
            let present_mode = {
                use self::hal::window::PresentMode::*;
                [Mailbox, Fifo, Relaxed, Immediate]
                    .iter()
                    .cloned()
                    .find(|pm| present_modes.contains(pm))
                    .ok_or("No PresentMode values specified!")?
            };
            let composite_alpha = {
                use self::hal::window::CompositeAlpha::*;
                [Opaque, Inherit, PreMultiplied, PostMultiplied]
                    .iter()
                    .cloned()
                    .find(|ca| composite_alphas.contains(ca))
                    .ok_or("No CompositeAlpha values specified!")?
            };
            let format = match preferred_formats {
                None => Format::Rgba8Srgb,
                Some(formats) => match formats
                    .iter()
                    .find(|format| format.base_format().1 == ChannelType::Srgb)
                    .cloned()
                {
                    Some(srgb_format) => srgb_format,
                    None => formats.get(0).cloned().ok_or("Preferred format list was empty!")?,
                },
            };
            let extent = {
              let screen_sz = w
                .get_inner_size()
                .ok_or("Window doesn't exist!")?
                .to_physical(w.get_hidpi_factor());
              Extent2D {
                width: caps.extents.end.width.min(screen_sz.width as u32),
                height: caps.extents.end.height.min(screen_sz.height as u32),
              }
            };
            let image_count = if present_mode == PresentMode::Mailbox {
                (caps.image_count.end - 1).min(3)
            } else {
                (caps.image_count.end - 1).min(2)
            };
            let image_layers = 1;
            let image_usage = if caps.usage.contains(Usage::COLOR_ATTACHMENT) {
                Usage::COLOR_ATTACHMENT
            } else {
                Err("The Surface isn't capable of supporting color!")?
            };
            let swapchain_cfg = SwapchainConfig {
                present_mode,
                composite_alpha,
                format,
                extent,
                image_count,
                image_layers,
                image_usage,
            };
            info!("{:?}", swapchain_cfg);
            //
            let (swapchain, backbuffer) = unsafe {
                device
                    .create_swapchain(&mut surf, swapchain_cfg, None)
                    .map_err(|_| "Failed to create the swapchain!")?
            };
            (swapchain, extent, backbuffer, format, image_count as usize)
        };
        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = {
            let mut image_available_semaphores = vec![];
            let mut render_finished_semaphores = vec![];
            let mut in_flight_fences = vec![];
            for _ in 0..max_frames_in_flight {
                in_flight_fences.push(device.create_fence(true).map_err(|_| "Could not create a fence!")?);
                image_available_semaphores.push(device.create_semaphore().map_err(|_| "Could not create a semaphore!")?);
                render_finished_semaphores.push(device.create_semaphore().map_err(|_| "Could not create a semaphore!")?);
            }
            (image_available_semaphores, render_finished_semaphores, in_flight_fences)
        };
        let mut render_pass = {
            let color_attachment = Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps {
                    load: AttachmentLoadOp::Clear,
                    store: AttachmentStoreOp::Store,
                },
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present,
            };
            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };
            unsafe {
                device
                    .create_render_pass(&[color_attachment], &[subpass], &[])
                    .map_err(|_| "Couldn't create a render pass!")?
            }
        };
        let loaded_buffers = vec![];
        let alloc_buffers = vec![
            AllocatedBuffer::new(
                &adapter, &device, None,
                (3 * geom::Vertex::packed_sz()) as u64,
                buffer::Usage::VERTEX
            )?,
            AllocatedBuffer::new(
                &adapter, &device, None,
                (3 * geom::Face::packed_sz()) as u64,
                buffer::Usage::INDEX
            )?,
            AllocatedBuffer::new(
                &adapter, &device, None,
                (3 * geom::Face::packed_sz()) as u64,
                buffer::Usage::INDEX
            )?
        ];
        let (descriptor_set_layouts, pipeline_layout, graphics_pipeline) =
            Self::create_pipeline(&mut device, extent, &mut render_pass)?;
        let image_views: Vec<_> = match backbuffer {
            Backbuffer::Images(images) => images
                .into_iter()
                .map(|image| unsafe {
                    device
                        .create_image_view(
                            &image,
                            ViewKind::D2,
                            format,
                            Swizzle::NO,
                            SubresourceRange {
                                aspects: Aspects::COLOR,
                                levels: 0..1,
                                layers: 0..1,
                            },
                        )
                        .map_err(|_| "Couldn't create the image_view for the image!")
                })
                .collect::<Result<Vec<_>, &str>>()?,
            Backbuffer::Framebuffer(_) => unimplemented!("Can't handle framebuffer backbuffer!"),
        };
        let framebuffers = {
            image_views
                .iter()
                .map(|image_view| unsafe {
                    device
                        .create_framebuffer(
                            &render_pass,
                            vec![image_view],
                            Extent {
                                width: extent.width as u32,
                                height: extent.height as u32,
                                depth: 1,
                            },
                        )
                        .map_err(|_| "Failed to create a framebuffer!")
                })
                .collect::<Result<Vec<_>, &str>>()?
        };
        let mut command_pool = unsafe {
            device
                .create_command_pool_typed(&queue_group, CommandPoolCreateFlags::RESET_INDIVIDUAL)
                .map_err(|_| "Could not create the raw command pool!")?
        };
        let command_buffers: Vec<_> = framebuffers.iter().map(|_| command_pool.acquire_command_buffer()).collect();
        Result::Ok(Renderer {
            current_frame: 0,
            max_frames_in_flight: max_frames_in_flight,

            in_flight_fences: in_flight_fences,
            render_finished_semaphores: render_finished_semaphores,
            image_available_semaphores: image_available_semaphores,

            command_buffers: command_buffers,
            command_pool: ManuallyDrop::new(command_pool),

            framebuffers: framebuffers,
            image_views: image_views,

            graphics_pipeline: ManuallyDrop::new(graphics_pipeline),
            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            descriptor_set_layouts: descriptor_set_layouts,

            alloc_buffers: alloc_buffers,
            loaded_buffers: loaded_buffers,

            render_pass: ManuallyDrop::new(render_pass),
            render_area: Extent::rect(&extent.to_extent()),
            queue_group: ManuallyDrop::new(queue_group),
            swapchain: ManuallyDrop::new(swapchain),
            device: ManuallyDrop::new(device),
            _adapter: adapter,
            _surface: surf,
            _instance: ManuallyDrop::new(inst),
        })
    }
    fn compile_shaders<'a, 'device>(
        device: &'device mut <I::Backend as Backend>::Device,
        mut shaders: Vec<ShaderInfo<'a>>,
    ) -> Result<Vec<CompiledShader<'a, I::Backend>>, &'static str> {
        let mut compiler = shaderc::Compiler::new().ok_or("shaderc not found!")?;
        let mut compiled_shaders = Vec::with_capacity(size_of::<CompiledShader<'a, I::Backend>>() * shaders.len());
        for si in shaders.drain(..) {
            compiled_shaders.push(CompiledShader::new(&mut compiler, device, si)?)
        }
        Ok(compiled_shaders)
    }
    fn destroy_shader_modules(
        device: &mut <I::Backend as Backend>::Device,
        mut modules: Vec<CompiledShader<I::Backend>>,
    ) {
        for module in modules.drain(..) {
            module.destroy(device);
        }
    }
    fn create_shaders<'a>(dev: &mut <I::Backend as Backend>::Device) -> Result<
        Vec<CompiledShader<'a, I::Backend>>, &'static str
    > {
        Self::compile_shaders(dev, vec![ShaderInfo {
            kind: shaderc::ShaderKind::Vertex,
            name: "basic.vert",
            entry_fn: "main",
            src: VERTEX_SOURCE,
            opts: None,
        }, ShaderInfo {
            kind: shaderc::ShaderKind::Fragment,
            name: "basic.frag",
            entry_fn: "main",
            src: FRAGMENT_SOURCE,
            opts: None,
        }])
    }
    fn vertex_attribs() -> Result<Vec<AttributeDesc>, &'static str> {
        let aa = geom::Vertex::attributes();
        let strides = geom::Vertex::packed_sz();
        let mut curr_loc = 0;
        let mut curr_binding = 0;
        let mut descs = Vec::new();
        for a in aa.iter() {
            trace!("Converting {:?} to AttributeDesc.", a);
            descs.push(AttributeDesc {
                location: curr_loc,
                binding: 0,
                element: Element {
                    format: match a.elemsize {
                        4 => Ok(Format::R32Float),
                        8 => Ok(Format::Rg32Float),
                        12 => Ok(Format::Rgb32Float),
                        _ => Err("Could not match size to format.")
                    }?,
                    offset: a.offset as u32,
                }
            });
            curr_loc += 1;
        }
        Ok(descs)
    }
    fn face_index_type() -> IndexType {
        // TODO do this smarter
        IndexType::U32
    }
    fn create_pipeline(
        device: &mut <I::Backend as Backend>::Device, extent: Extent2D, render_pass: &<<I>::Backend as Backend>::RenderPass,
    ) -> Result<(
        Vec<<<I>::Backend as Backend>::DescriptorSetLayout>,
        <<I>::Backend as Backend>::PipelineLayout,
        <<I>::Backend as Backend>::GraphicsPipeline,
    ), &'static str> {
        let compiled_shaders = Self::create_shaders(device)?;
        let shaders = GraphicsShaderSet {
            vertex: compiled_shaders[0].get_entry(Specialization {
                constants: &[],
                data: &[],
            }),
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(compiled_shaders[1].get_entry(Specialization {
                constants: &[],
                data: &[],
            })),
        };
        let vertex_buffers: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
            binding: 0,
            stride: geom::Vertex::packed_sz() as ElemStride,
            rate: 0,
        }];
        let attributes = Self::vertex_attribs()?;
        let input_assembler = InputAssemblerDesc::new(Primitive::TriangleList);
        let rasterizer = Rasterizer {
            depth_clamping: false,
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::NONE,
            front_face: FrontFace::Clockwise,
            depth_bias: None,
            conservative: false,
        };
        let depth_stencil = pso::DepthStencilDesc {
            depth: DepthTest::Off,
            depth_bounds: false,
            stencil: StencilTest::Off,
        };
        let blender = {
            let blend_state = BlendState::On {
                color: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
                alpha: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
            };
            BlendDesc {
                logic_op: Some(LogicOp::Copy),
                targets: vec![ColorBlendDesc(ColorMask::ALL, blend_state)],
            }
        };
        let baked_states = BakedStates {
            viewport: Some(Viewport {
                rect: extent.to_extent().rect(),
                depth: (0.0..1.0),
            }),
            scissor: Some(extent.to_extent().rect()),
            blend_color: None,
            depth_bounds: None,
        };
        let bindings = Vec::<DescriptorSetLayoutBinding>::new();
        let immutable_samplers = Vec::<<I::Backend as Backend>::Sampler>::new();
        let descriptor_set_layouts: Vec<<I::Backend as Backend>::DescriptorSetLayout> = vec![unsafe {
            device.create_descriptor_set_layout(bindings, immutable_samplers)
                .map_err(|_| "Couldn't make a DescriptorSetLayout")?
        }];
        let push_constants: Vec<(ShaderStageFlags, std::ops::Range<u32>)> = vec![
            (ShaderStageFlags::FRAGMENT, 0..4)
        ];
        let layout = unsafe {
            device.create_pipeline_layout(&descriptor_set_layouts, push_constants)
                .map_err(|_| "Couldn't create a pipeline layout")?
        };
        /****/
        let gp = {
            let desc = GraphicsPipelineDesc {
                shaders,
                rasterizer,
                vertex_buffers,
                attributes,
                input_assembler,
                blender,
                depth_stencil,
                multisampling: None,
                baked_states,
                layout: &layout,
                subpass: Subpass {
                  index: 0,
                  main_pass: render_pass,
                },
                flags: PipelineCreationFlags::empty(),
                parent: BasePipeline::None,
            };
            unsafe {
                device.create_graphics_pipeline(&desc, None)
                    .map_err(|_| {"Couldn't create a graphics pipeline!"})?
            }
        };
        Self::destroy_shader_modules(device, compiled_shaders);
        Result::Ok((descriptor_set_layouts, layout, gp))
    }
    fn draw_empty_scene(&mut self) -> Result<(), &'static str> {
        let since_epoch = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => (((n.as_nanos() % 1_000_000_000u128) as f64) / 1_000_000_000f64) as f32,
            Err(_) => 1f32
        };
        let col = Vector4::repeat(since_epoch);
        self.clear_color(col)
    }
    fn clear_color<C>(&mut self, color: C) -> Result<(), &'static str> where C: Into<[f32; 4]> {
        let color = color.into();
        // TODO FRAME PREP
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;

        let (img_idx_u32, img_idx_usize) = unsafe {
            let image_index = self
                .swapchain
                .acquire_image(core::u64::MAX, FrameSync::Semaphore(image_available))
                .map_err(|_| "Couldn't acquire an image from the swapchain!")?;
            (image_index, image_index as usize)
        };
        let flight_fence = &self.in_flight_fences[img_idx_usize];
        unsafe {
            self.device
                .wait_for_fence(flight_fence, core::u64::MAX)
                .map_err(|_| "Failed to wait on the fence!")?;
            self.device
                .reset_fence(flight_fence)
                .map_err(|_| "Couldn't reset the fence!")?;
        }

        // TODO RECORD COMMANDS
        unsafe {
            let buffer = &mut self.command_buffers[img_idx_usize];
            let clear_values = [ClearValue::Color(ClearColor::Float(color))];
            buffer.begin(false);
            buffer.begin_render_pass_inline(
                &self.render_pass,
                &self.framebuffers[img_idx_usize],
                self.render_area,
                clear_values.iter(),
            );
            buffer.finish();
        }
        // TODO SUBMIT
        let command_buffers = &self.command_buffers[img_idx_usize..=img_idx_usize];
        let wait_semaphores: ArrayVec<[_; 1]> = [(image_available, PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
        let signal_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        let present_wait_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        let submission = Submission {
          command_buffers,
          wait_semaphores,
          signal_semaphores,
        };
        let the_command_queue = &mut self.queue_group.queues[0];
        unsafe {
          the_command_queue.submit(submission, Some(flight_fence));
          self.swapchain.present(the_command_queue, img_idx_u32, present_wait_semaphores)
            .map_err(|_| "Failed to present into the swapchain!")
        }
    }
    fn draw_geom<C: Into<[f32; 4]>>(&mut self, m: geom::Model, color: C) -> Result<(), &'static str> {
        // FRAME PREP
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
        let (img_idx_u32, img_idx_usize) = unsafe {
            let image_index = self
                .swapchain
                .acquire_image(core::u64::MAX, FrameSync::Semaphore(image_available))
                .map_err(|_| "Couldn't acquire an image from the swapchain!")?;
            (image_index, image_index as usize)
        };
        let flight_fence = &self.in_flight_fences[img_idx_usize];
        unsafe {
            self.device
                .wait_for_fence(flight_fence, core::u64::MAX)
                .map_err(|_| "Failed to wait on the fence!")?;
            self.device
                .reset_fence(flight_fence)
                .map_err(|_| "Couldn't reset the fence!")?;
        }
        // WRITE THE CHANGED DATA
        let index_buffer = {
            let mut found_buffer = None;
            // TODO use a pointer map later
            for b in self.loaded_buffers.iter() {
                if b.matches_source(&m.source) {
                    found_buffer = Some(b);
                    break;
                }
            }
            if let Some(b) = found_buffer { b } else {
                let ff_bytes = m.ff_as_bytes();
                self.loaded_buffers.push(LoadedBuffer::new(
                    &self._adapter, &mut self.device,
                    None,
                    (&**m.source).packed_ff_sz() as u64, buffer::Usage::INDEX,
                    &m.ff_as_bytes(), m.source.clone()
                )?);
                self.loaded_buffers.last().expect("Loaded buffer that was just pushed does not exist.")
            }
        };
        let vert_buffer = {
            self.alloc_buffers[0].load_data_from_slice(&mut self.device, &m.vv_as_bytes(), 0)?;
            &self.alloc_buffers[0]
        };
        // RECORD COMMANDS
        unsafe {
            let buffer = &mut self.command_buffers[img_idx_usize];
            const TRIANGLE_CLEAR: [ClearValue; 1] = [
                ClearValue::Color(ClearColor::Float([0f32, 0f32, 0f32, 1.0]))
            ];
            buffer.begin(false);
            {
                let mut encoder = buffer.begin_render_pass_inline(
                    &self.render_pass,
                    &self.framebuffers[img_idx_usize],
                    self.render_area,
                    TRIANGLE_CLEAR.iter(),
                );
                encoder.bind_graphics_pipeline(&self.graphics_pipeline);
                // Here we must force the Deref impl of ManuallyDrop to play nice.
                let vert_buffer_ref: &<I::Backend as Backend>::Buffer = &vert_buffer.buffer_ref();
                let buffers: ArrayVec<[_; 1]> = [(vert_buffer_ref, 0)].into();
                encoder.bind_vertex_buffers(0, buffers);
                encoder.bind_index_buffer(buffer::IndexBufferView {
                    buffer: index_buffer.buffer_ref(),
                    offset: 0,
                    index_type: Self::face_index_type(),
                });
                encoder.push_graphics_constants(
                    &self.pipeline_layout,
                    ShaderStageFlags::FRAGMENT,
                    0,
                    &Self::as_buffer(&color.into())
                );
                encoder.draw_indexed(0..(m.source.packed_ff_sz() as u32), 0, 0..1);
            }
            buffer.finish();
        }
        // SUBMIT COMMANDS
        let command_buffers = &self.command_buffers[img_idx_usize..=img_idx_usize];
        let wait_semaphores: ArrayVec<[_; 1]> = [(image_available, PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
        let signal_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        let present_wait_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        let submission = Submission {
            command_buffers,
            wait_semaphores,
            signal_semaphores,
        };
        let the_command_queue = &mut self.queue_group.queues[0];
        unsafe {
            the_command_queue.submit(submission, Some(flight_fence));
            self.swapchain.present(the_command_queue, img_idx_u32, present_wait_semaphores)
                .map_err(|_| "Failed to present into the swapchain!")
        }
    }
    fn as_buffer(v: &[f32; 4]) -> [u32; 4] {
        let mut av: [u32; 4] = unsafe { std::mem::uninitialized() };
        for (i, seg) in v.iter().enumerate() {
            av[i] = seg.to_bits();
        }
        av
    }
    fn handle_req(r: &mut Renderer<I>, q: RenderReq<I>) -> Result<(), &'static str> {
        match q {
            RenderReq::Clear(Color(c)) => r.clear_color(c),
            RenderReq::Draw(m, Color(c)) => r.draw_geom(m, c),
            RenderReq::Free(a) => match a.lock() {
                Ok(mut f) => Result::Ok(f.deref_mut()(r)),
                Err(_) => Result::Err("I hate when I'm given poisoned cookies."),
            },
            RenderReq::Seq(qq) => {
                for q in qq {
                    Self::handle_req(r, q)?
                }
                Result::Ok(())
            }
            RenderReq::Restart => Result::Err("Cannot directly handle restarts!"),
        }
    }
}
impl <I: Instance> Drop for Renderer<I> {
    fn drop(&mut self) {
        self.device.wait_idle().expect("Welp, guess we can't do anything anymore. So I'll just panic here.");
        unsafe {
            for fence in self.in_flight_fences.drain(..) {
                self.device.destroy_fence(fence)
            }
            for s in self.render_finished_semaphores.drain(..) {
                self.device.destroy_semaphore(s)
            }
            for s in self.image_available_semaphores.drain(..) {
                self.device.destroy_semaphore(s)
            }

            self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&mut self.graphics_pipeline)));
            self.device.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&mut self.pipeline_layout)));
            for dsl in self.descriptor_set_layouts.drain(..) {
                self.device.destroy_descriptor_set_layout(dsl);
            }

            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb)
            }
            for iv in self.image_views.drain(..) {
                self.device.destroy_image_view(iv)
            }

            for mut b in self.alloc_buffers.drain(..) {
                b.free(&self.device);
            }
            for mut b in self.loaded_buffers.drain(..) {
                b.free(&self.device);
            }

            // The CommandPool must also be unwrapped into a RawCommandPool,
            // so there's an extra `into_raw` call here.
            self.device.destroy_command_pool(ManuallyDrop::into_inner(read(&mut self.command_pool)).into_raw());
            self.device.destroy_render_pass(
                ManuallyDrop::into_inner(read(&mut self.render_pass))
            );
            self.device.destroy_swapchain(
                ManuallyDrop::into_inner(read(&mut self.swapchain))
            );
            ManuallyDrop::drop(&mut self.graphics_pipeline);
            ManuallyDrop::drop(&mut self.pipeline_layout);
            ManuallyDrop::drop(&mut self.queue_group);
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self._instance);
        }
    }
}

pub struct RenderStage<I: Instance> {
    req_tx: Sender<RenderReq<I>>,
    // update_rx: Receiver<geom::Frame>,
    scene: Arc<RwLock<Option<Scene>>>,
    render_thread: Option<KillableThread<()>>,
}
impl <I: Instance> RenderStage<I> {
    fn start_render_thread<F: RendererCreator<I>>(req_rx: Receiver<RenderReq<I>>, w: Arc<Window>, f: F) -> KillableThread<()> {
        th::create_kt!((), "Render Stage", {
            let mut r = f(&*w).expect("Fuck. Couldn't even create a thingy-thing.");
        }, {
            match req_rx.try_recv() {
                Ok(req) => if let RenderReq::Restart = req {
                    drop(r);
                    r = f(&*w).expect("Fuck. Couldn't create a thingy-thing after the first time.");
                } else {
                    match Renderer::handle_req(&mut r, req) {
                        Ok(_) => (),
                        Err(s) => {
                            error!("Error ({:?}) while handling request. Attempting recovery...", s);
                            drop(r);
                            r = f(&*w).expect("Fuck. Couldn't create a thingy-thing after the first time.");
                        }
                    }
                },
                Err(TryRecvError::Disconnected) => warn!("Request channel lost prior to shutdown!"),
                Err(TryRecvError::Empty) => (),
            }
        }, {}).expect("Could not start render thread.... Welp I'm out.")
    }
    fn new<F: RendererCreator<I>>(sc_arc: Arc<RwLock<Option<Scene>>>, w: Arc<Window>, f: F) -> RenderStage<I> {
        let (req_tx, req_rx) = channel();
        RenderStage {
            req_tx: req_tx,
            scene: sc_arc,
            render_thread: Option::Some(Self::start_render_thread(req_rx, w, f)),
        }
    }
    pub fn send_cmd(&self, q: RenderReq<I>) -> Result<(), SendError<RenderReq<I>>> { self.req_tx.send(q) }
    pub fn finish(mut self) -> FinishResult {
        self.render_thread.take().map_or_else(|| Option::None, |kt| kt.finish())
    }
}
pub type FinishResult = super::kt::FinishResult<()>;
impl <I: Instance> Drop for RenderStage<I> {
    fn drop(&mut self) {
        if self.render_thread.is_some() {
            panic!("Must call finish on RenderStage before dropping.");
        }
    }
}

pub type IT = back::Instance;
pub type TypedRenderer = Renderer<IT>;
pub type TypedRenderReq = RenderReq<IT>;
pub type TypedRenderStage = RenderStage<IT>;
impl TypedRenderer {
    fn create(w: &Window) -> Result<TypedRenderer, &'static str> {
        let inst = back::Instance::create("Tracer", 1);
        let surf = inst.create_surface(w);
        TypedRenderer::new(w, inst, surf)
    }
}
impl TypedRenderStage {
    pub fn create(sc_arc: Arc<RwLock<Option<Scene>>>, w: Arc<Window>) -> TypedRenderStage {
        TypedRenderStage::new(sc_arc, w, |w: &Window| -> Result<TypedRenderer, &'static str> {
            TypedRenderer::create(w)
        })
    }
}

