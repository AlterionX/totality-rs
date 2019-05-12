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
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
extern crate totality_threading as th;
extern crate totality_model as model;
extern crate arrayvec as av;
extern crate nalgebra as na;
extern crate image as img;

mod shaders;
mod buffers;
mod texture;

use std::{
    ops::DerefMut,
    time::SystemTime,
    sync::{Arc, Mutex, RwLock, mpsc::{Sender, Receiver, channel, SendError}},
    mem::{size_of, ManuallyDrop},
    ptr::read,
};
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
use av::ArrayVec;
use na::Vector4;
use winit::Window;
use th::killable_thread::{self as kt, KillableThread};
use model::{self as geom, scene::Scene};
use buffers::{AllocatedBuffer, LoadedBuffer};
use shaders::{ShaderInfo, CompiledShader};
use texture::LoadedImage;

#[allow(dead_code)]
use log::{error, warn, info, debug, trace};

const VERTEX_SOURCE: &str = include_str!("../resources/shaders/basic.vert");
const FRAGMENT_SOURCE: &str =  include_str!("../resources/shaders/basic.frag");

pub trait RendererCreator<I: Instance>: Fn(&Window) -> Result<Renderer<I>, &'static str> + Send + 'static {}
impl <F: Fn(&Window) -> Result<Renderer<I>, &'static str> + Send + 'static, I: Instance> RendererCreator<I> for F {}
pub trait RenderFn<I: Instance>: FnMut(&mut Renderer<I>) + Send + 'static {}
impl <F: FnMut(&mut Renderer<I>) + Send + 'static, I: Instance> RenderFn<I> for F {}
pub struct Color(pub Vector4<f32>);
pub struct RenderSettings {
    pub should_use_depth: bool,
}
pub enum RenderReq<I: Instance> {
    Restart,
    Clear(Color),
    Seq(Vec<RenderReq<I>>),
    DrawDirect(geom::Model, geom::camera::Camera, Color),
    Draw(geom::Model, geom::camera::Camera, Color),
    DrawGroup(Vec<geom::Model>, geom::camera::Camera, Color),
    DrawGroupWithSetting(Vec<geom::Model>, geom::camera::Camera, Color, RenderSettings),
    Free(Arc<Mutex<RenderFn<I>>>),
}

pub struct Renderer<I: Instance> {
    current_frame: usize,
    max_frames_in_flight: usize,

    in_flight_fences: Vec<<I::Backend as Backend>::Fence>,
    render_finished_semaphores: Vec<<I::Backend as Backend>::Semaphore>,
    image_available_semaphores: Vec<<I::Backend as Backend>::Semaphore>,

    command_buffers: Vec<CommandBuffer<I::Backend, Graphics, MultiShot, Primary>>,
    command_pool: ManuallyDrop<CommandPool<I::Backend, Graphics>>,

    framebuffers: Vec<<I::Backend as Backend>::Framebuffer>,
    framebuffers_no_depth: Vec<<I::Backend as Backend>::Framebuffer>,
    depth_buffers: Vec<texture::DepthImage<I::Backend>>,
    image_views: Vec<(<I::Backend as Backend>::ImageView)>,

    instance_model_buffers: Vec<AllocatedBuffer<I::Backend>>,
    loaded_images: Vec<LoadedImage<I::Backend>>,
    alloc_buffers: Vec<AllocatedBuffer<I::Backend>>,
    loaded_buffers: Vec<LoadedBuffer<Box<geom::Geom>, I::Backend>>,

    descriptor_sets: Vec<<I::Backend as Backend>::DescriptorSet>,
    descriptor_pool: ManuallyDrop<<I::Backend as Backend>::DescriptorPool>,

    graphics_pipeline: ManuallyDrop<<I::Backend as Backend>::GraphicsPipeline>,
    graphics_pipeline_no_depth: ManuallyDrop<<I::Backend as Backend>::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<<I::Backend as Backend>::PipelineLayout>,
    descriptor_set_layouts: Vec<<I::Backend as Backend>::DescriptorSetLayout>,

    render_pass: ManuallyDrop<<I::Backend as Backend>::RenderPass>,
    render_pass_no_depth: ManuallyDrop<<I::Backend as Backend>::RenderPass>,
    render_area: Rect,
    queue_group: ManuallyDrop<QueueGroup<I::Backend, Graphics>>,
    swapchain: ManuallyDrop<<I::Backend as Backend>::Swapchain>,

    device: ManuallyDrop<<I::Backend as Backend>::Device>,
    _adapter: Adapter<I::Backend>,
    _surface: <I::Backend as Backend>::Surface,
    _instance: ManuallyDrop<I>,
}
impl<B: Backend<Device=D>, D: Device<B>, I: Instance<Backend=B>> Renderer<I> {
    pub const MAX_INSTANCE_COUNT: usize = 5_000_000; // TODO set this dynamically
    fn new(w: &Window, inst: I, mut surf: B::Surface) -> Result<Self, &'static str> {
        let adapter = hal::Instance::enumerate_adapters(&inst).into_iter().find(|a| {
           a.queue_families.iter()
               .any(|qf| qf.supports_graphics() && surf.supports_queue_family(qf))
        }).ok_or("Couldn't find a graphical Adapter!")?;
        let (mut device, queue_group) = {
            let queue_family = adapter.queue_families.iter().find(|qf| qf.supports_graphics() && surf.supports_queue_family(qf))
                .ok_or("Couldn't find a QueueFamily with graphics!")?;
            let Gpu { device, mut queues } = unsafe {
                use hal::Features;
                adapter.physical_device.open(&[(&queue_family, &[1.0; 1])], Features::empty())
                    .map_err(|_| "Couldn't open the PhysicalDevice!")?
            };
            let queue_group = queues.take::<Graphics>(queue_family.id())
                .ok_or("Couldn't take ownership of the QueueGroup!")?;
            let _ = if queue_group.queues.len() > 0 { Ok(()) } else { Err("The QueueGroup did not have any CommandQueues available!") }?;
            (device, queue_group)
        };
        let (swapchain, extent, backbuffers, format, max_frames_in_flight) = {
            let (caps, preferred_formats, present_modes) = surf.compatibility(&adapter.physical_device);
            info!("{:?}", caps);
            info!("Preferred Formats: {:?}", preferred_formats);
            info!("Present Modes: {:?}", present_modes);
            info!("Composite Alphas: {:?}", caps.composite_alpha);
            let present_mode = {
                use self::hal::window::PresentMode::*;
                [Mailbox, Fifo, Relaxed, Immediate]
                    .iter()
                    .cloned()
                    .find(|pm| present_modes.contains(pm))
                    .ok_or("No PresentMode values specified!")?
            };
            let composite_alpha = {
                use hal::window::CompositeAlpha;
                [CompositeAlpha::OPAQUE, CompositeAlpha::INHERIT, CompositeAlpha::PREMULTIPLIED, CompositeAlpha::POSTMULTIPLIED].iter().cloned().find(
                    |ca| caps.composite_alpha.contains(*ca)
                ).ok_or("No CompositeAlpha values specified!")?
            };
            let format = match preferred_formats {
                None => Format::Rgba8Srgb,
                Some(formats) => match formats.iter().find(|format| format.base_format().1 == ChannelType::Srgb).cloned() {
                    Some(srgb_format) => srgb_format,
                    None => formats.get(0).cloned().ok_or("Preferred format list was empty!")?,
                },
            };
            let extent = {
                let screen_sz = w.get_inner_size().ok_or("Window doesn't exist!")?.to_physical(w.get_hidpi_factor());
                Extent2D {
                    width: caps.extents.end.width.min(screen_sz.width as u32),
                    height: caps.extents.end.height.min(screen_sz.height as u32),
                }
            };
            info!("Framebuffer target size: {:?}.", extent);
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
            let (swapchain, backbuffers) = unsafe {
                device.create_swapchain(&mut surf, swapchain_cfg, None)
                    .map_err(|_| "Failed to create the swapchain!")?
            };
            (swapchain, extent, backbuffers, format, image_count as usize)
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
            let depth_attachment = Attachment {
                format: Some(hal::format::Format::D32Sfloat),
                samples: 1,
                ops: AttachmentOps {
                    load: AttachmentLoadOp::Clear,
                    store: AttachmentStoreOp::DontCare,
                },
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::DepthStencilAttachmentOptimal,
            };
            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: Some(&(1, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };
            let in_dependency = SubpassDependency {
                passes: SubpassRef::External..SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT | PipelineStage::EARLY_FRAGMENT_TESTS,
                accesses: Access::empty()
                    ..(Access::COLOR_ATTACHMENT_READ
                    | Access::COLOR_ATTACHMENT_WRITE
                    | Access::DEPTH_STENCIL_ATTACHMENT_READ
                    | Access::DEPTH_STENCIL_ATTACHMENT_WRITE),
            };
            let out_dependency = SubpassDependency {
                passes: SubpassRef::Pass(0)..SubpassRef::External,
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT | PipelineStage::EARLY_FRAGMENT_TESTS
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: (Access::COLOR_ATTACHMENT_READ
                    | Access::COLOR_ATTACHMENT_WRITE
                    | Access::DEPTH_STENCIL_ATTACHMENT_READ
                    | Access::DEPTH_STENCIL_ATTACHMENT_WRITE)..Access::empty(),
            };
            unsafe { device.create_render_pass(
                    &[color_attachment, depth_attachment],
                    &[subpass],
                    &[in_dependency, out_dependency],
            ).map_err(|_| "Couldn't create a render pass!")? }
        };
        let mut render_pass_no_depth = {
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
                depth_stencil: None, //Some(&(1, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };
            unsafe { device.create_render_pass(
                    &[color_attachment],
                    &[subpass],
                    &[],
            ).map_err(|_| "Couldn't create a render pass!")? }
        };
        let loaded_buffers = vec![];
        let alloc_buffers = vec![AllocatedBuffer::new(
            &adapter, &device, None,
            (8 * geom::Vertex::packed_byte_sz()) as u64,
            buffer::Usage::VERTEX
        )?];
        let instance_model_buffers = (0..3).map(|_| AllocatedBuffer::new(
                &adapter, &device, None,
                (16 * size_of::<f32>() * Self::MAX_INSTANCE_COUNT) as u64,
                buffer::Usage::VERTEX,
        )).collect::<Result<_, &str>>()?;
        let (descriptor_set_layouts, pipeline_layout, graphics_pipeline, graphics_pipeline_no_depth) =
            Self::create_pipeline(&mut device, extent, &mut render_pass, &mut render_pass_no_depth)?;
        let mut descriptor_pool = unsafe { device.create_descriptor_pool(
            max_frames_in_flight, // sets
            &[gfx_hal::pso::DescriptorRangeDesc {
                ty: gfx_hal::pso::DescriptorType::SampledImage,
                count: max_frames_in_flight,
            }, gfx_hal::pso::DescriptorRangeDesc {
                ty: gfx_hal::pso::DescriptorType::Sampler,
                count: max_frames_in_flight,
            }],
            DescriptorPoolCreateFlags::empty(),
        ).map_err(|_| "Couldn't create a descriptor pool!")? };
        let descriptor_sets = {
            let mut sets = Vec::with_capacity(max_frames_in_flight);
            for set_i in 0..max_frames_in_flight {
                unsafe { match descriptor_pool.allocate_set(&descriptor_set_layouts[0]) {
                    Ok(o) => sets.push(o),
                    e @ Err(_) => {
                        error!("{:?}", e);
                        Err("Couldn't make a Descriptor Set!")?
                    }
                } }
            }
            sets
        };
        let image_views: Vec<_> = backbuffers.into_iter().map(|image| unsafe {
            device.create_image_view(
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
        }).collect::<Result<Vec<_>, &str>>()?;
        let depth_buffers = image_views.iter().map(|_| {
            texture::DepthImage::new(&adapter, &device, extent)
        }).collect::<Result<Vec<_>, &str>>()?;
        let framebuffers = { image_views.iter().zip(depth_buffers.iter()).map(|(iv, db)| unsafe {
            device.create_framebuffer(
                &render_pass,
                vec![iv, db.img_view_ref()],
                Extent {
                    width: extent.width as u32,
                    height: extent.height as u32,
                    depth: 1,
                },
            )
            .map_err(|_| "Failed to create a framebuffer!")
        }).collect::<Result<Vec<_>, &str>>()?};
        let framebuffers_no_depth = { image_views.iter().map(|iv| unsafe {
            device.create_framebuffer(
                &render_pass_no_depth,
                vec![iv],
                Extent {
                    width: extent.width as u32,
                    height: extent.height as u32,
                    depth: 1,
                },
            )
            .map_err(|_| "Failed to create a framebuffer!")
        }).collect::<Result<Vec<_>, &str>>()?};
        let mut command_pool = unsafe {
            device
                .create_command_pool_typed(&queue_group, CommandPoolCreateFlags::RESET_INDIVIDUAL)
                .map_err(|_| "Could not create the raw command pool!")?
        };
        let command_buffers: Vec<_> = (0..max_frames_in_flight).map(|_| command_pool.acquire_command_buffer()).collect();
        // 4. You create the actual descriptors which you want to write into the
        //    allocated descriptor set (in this case an image and a sampler)
        let loaded_images = vec![];
        Result::Ok(Renderer {
            current_frame: 0,
            max_frames_in_flight: max_frames_in_flight,

            in_flight_fences: in_flight_fences,
            render_finished_semaphores: render_finished_semaphores,
            image_available_semaphores: image_available_semaphores,

            command_buffers: command_buffers,
            command_pool: ManuallyDrop::new(command_pool),

            framebuffers: framebuffers,
            framebuffers_no_depth: framebuffers_no_depth,
            depth_buffers: depth_buffers,
            image_views: image_views,

            descriptor_sets: descriptor_sets,
            descriptor_pool: ManuallyDrop::new(descriptor_pool),

            graphics_pipeline: ManuallyDrop::new(graphics_pipeline),
            graphics_pipeline_no_depth: ManuallyDrop::new(graphics_pipeline_no_depth),
            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            descriptor_set_layouts: descriptor_set_layouts,

            instance_model_buffers: instance_model_buffers,
            loaded_images: loaded_images,
            alloc_buffers: alloc_buffers,
            loaded_buffers: loaded_buffers,

            render_pass: ManuallyDrop::new(render_pass),
            render_pass_no_depth: ManuallyDrop::new(render_pass_no_depth),
            render_area: Extent::rect(&extent.to_extent()),
            queue_group: ManuallyDrop::new(queue_group),
            swapchain: ManuallyDrop::new(swapchain),
            device: ManuallyDrop::new(device),
            _adapter: adapter,
            _surface: surf,
            _instance: ManuallyDrop::new(inst),
        })
    }
    fn layout_set_descs(dev: &mut D) -> Result<Vec<B::DescriptorSetLayout>, &'static str> {
        let bindings = vec![DescriptorSetLayoutBinding {
            binding: 0,
            ty: gfx_hal::pso::DescriptorType::SampledImage,
            count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
            immutable_samplers: false,
        }, DescriptorSetLayoutBinding {
            binding: 1,
            ty: gfx_hal::pso::DescriptorType::Sampler,
            count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
            immutable_samplers: false,
        }];
        let immutable_samplers = vec![];
        let descriptor_set_layouts = vec![unsafe {
            dev.create_descriptor_set_layout(&bindings, &immutable_samplers)
                .map_err(|_| "Couldn't make a DescriptorSetLayout")?
        }];
        Ok(descriptor_set_layouts)
    }
    fn compile_shaders<'a, 'device>(device: &'device mut D, mut shaders: Vec<ShaderInfo<'a>>)
        -> Result<Vec<CompiledShader<'a, B>>, &'static str>
    {
        let mut compiler = shaderc::Compiler::new().ok_or("shaderc not found!")?;
        let mut compiled_shaders = Vec::with_capacity(size_of::<CompiledShader<'a, B>>() * shaders.len());
        for si in shaders.drain(..) {
            compiled_shaders.push(CompiledShader::new(&mut compiler, device, si)?)
        }
        Ok(compiled_shaders)
    }
    fn destroy_shader_modules(device: &mut D, mut modules: Vec<CompiledShader<B>>) {
        for module in modules.drain(..) {
            module.destroy(device);
        }
    }
    fn create_shaders<'a>(dev: &mut D) -> Result<Vec<CompiledShader<'a, B>>, &'static str> {
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
        let mut curr_loc = 0;
        let mut descs = Vec::new();
        for a in aa.iter() {
            trace!("Converting {:?} to AttributeDesc.", a);
            descs.push(AttributeDesc {
                location: curr_loc,
                binding: 0,
                element: Element {
                    format: match a.elemsize {
                        4 => Ok(Format::R32Sfloat),
                        8 => Ok(Format::Rg32Sfloat),
                        12 => Ok(Format::Rgb32Sfloat),
                        _ => Err("Could not match size to format.")
                    }?,
                    offset: a.offset as u32,
                }
            });
            curr_loc += 1;
        }
        for i in 0..4 {
            descs.push(AttributeDesc {
                location: curr_loc,
                binding: 1,
                element: Element {
                    format: Format::Rgba32Sfloat,
                    offset: (i * 4 * size_of::<f32>()) as u32,
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
    fn create_pipeline(device: &mut D, extent: Extent2D, render_pass: &B::RenderPass, render_pass_no_depth: &B::RenderPass) -> Result<(
        Vec<B::DescriptorSetLayout>, B::PipelineLayout, B::GraphicsPipeline, B::GraphicsPipeline,
    ), &'static str> {
        let compiled_shaders = Self::create_shaders(device)?;
        let shaders = GraphicsShaderSet {
            vertex: compiled_shaders[0].get_entry(),
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(compiled_shaders[1].get_entry()),
        };
        let shaders_no_depth = GraphicsShaderSet {
            vertex: compiled_shaders[0].get_entry(),
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(compiled_shaders[1].get_entry()),
        };
        let vertex_buffers: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
            binding: 0,
            stride: geom::Vertex::packed_byte_sz() as ElemStride,
            rate: VertexInputRate::Vertex,
        }, VertexBufferDesc {
            binding: 1,
            stride: (size_of::<f32>() * 16) as ElemStride,
            rate: VertexInputRate::Instance(1),
        }];
        let vertex_buffers_no_depth: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
            binding: 0,
            stride: geom::Vertex::packed_byte_sz() as ElemStride,
            rate:  VertexInputRate::Vertex,
        }, VertexBufferDesc {
            binding: 1,
            stride: (size_of::<f32>() * 16) as ElemStride,
            rate:  VertexInputRate::Instance(1),
        }];
        let attributes = Self::vertex_attribs()?;
        let attributes_no_depth = Self::vertex_attribs()?;
        let input_assembler = InputAssemblerDesc::new(Primitive::TriangleList);
        let input_assembler_no_depth = InputAssemblerDesc::new(Primitive::TriangleList);
        let rasterizer = Rasterizer {
            depth_clamping: false,
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::BACK,
            front_face: FrontFace::Clockwise,
            depth_bias: None,
            conservative: false,
        };
        let rasterizer_no_depth = Rasterizer {
            depth_clamping: false,
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::BACK,
            front_face: FrontFace::Clockwise,
            depth_bias: None,
            conservative: false,
        };
        let depth_stencil = pso::DepthStencilDesc {
            depth: DepthTest::On {
                fun: gfx_hal::pso::Comparison::LessEqual,
                write: true,
            },
            depth_bounds: false,
            stencil: StencilTest::Off,
        };
        let depth_stencil_no_depth = pso::DepthStencilDesc {
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
        let blender_no_depth = {
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
        let baked_states_no_depth = BakedStates {
            viewport: Some(Viewport {
                rect: extent.to_extent().rect(),
                depth: (0.0..1.0),
            }),
            scissor: Some(extent.to_extent().rect()),
            blend_color: None,
            depth_bounds: None,
        };
        let descriptor_set_layouts = Self::layout_set_descs(device)?;
        let push_constants: Vec<(ShaderStageFlags, std::ops::Range<u32>)> = vec![
            (ShaderStageFlags::VERTEX, 0..16),
            (ShaderStageFlags::FRAGMENT, 16..24),
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
        let gp_no_depth = {
            let desc = GraphicsPipelineDesc {
                shaders:        shaders_no_depth,
                rasterizer:     rasterizer_no_depth,
                vertex_buffers: vertex_buffers_no_depth,
                attributes:     attributes_no_depth,
                input_assembler:input_assembler_no_depth,
                blender:        blender_no_depth,
                depth_stencil: depth_stencil_no_depth,
                multisampling: None,
                baked_states: baked_states_no_depth,
                layout: &layout,
                subpass: Subpass {
                  index: 0,
                  main_pass: render_pass_no_depth,
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
        Result::Ok((descriptor_set_layouts, layout, gp, gp_no_depth))
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
            let (image_index, _optimality) = self.swapchain.acquire_image(core::u64::MAX, Some(image_available), None)
                .map_err(|_| "Couldn't acquire an image from the swapchain!")?;
            (image_index, image_index as usize)
        };
        let flight_fence = &self.in_flight_fences[img_idx_usize];
        unsafe {
            self.device.wait_for_fence(flight_fence, core::u64::MAX)
                .map_err(|_| "Failed to wait on the fence!")?;
            self.device.reset_fence(flight_fence)
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
          let _optimality = self.swapchain.present(the_command_queue, img_idx_u32, present_wait_semaphores)
            .map_err(|_| "Failed to present into the swapchain!")?;
        };
        Ok(())
    }
    fn draw_instanced_geom<C: Into<[f32; 4]>>(&mut self, mm: Vec<geom::Model>, cam: geom::camera::Camera, color: C) -> Result<(), &'static str> {
        // FRAME PREP
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
        let (img_idx_u32, img_idx_usize) = unsafe {
            let (image_index, _optimality) = self.swapchain.acquire_image(core::u64::MAX, Some(image_available), None)
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
        // LOAD ALL NEEDED DATA
        let vert_buffer = {
            let ref vb = self.alloc_buffers[0];
            vb.load_data_from_slice(&mut self.device, &mm[0].vv_as_bytes(), 0)?;
            vb
        };
        let index_buffer = {
            let mut found_buffer = None;
            // TODO use a pointer map for O(1)ish look up later
            for b in self.loaded_buffers.iter() {
                if b.matches_source(&mm[0].source) {
                    found_buffer = Some(b);
                    break;
                }
            }
            if let Some(b) = found_buffer { b } else {
                self.loaded_buffers.push(LoadedBuffer::new(
                    &self._adapter, &mut self.device,
                    None,
                    (&**mm[0].source).ff_byte_cnt() as u64, buffer::Usage::INDEX,
                    &mm[0].ff_as_bytes(), mm[0].source.clone()
                )?);
                self.loaded_buffers.last().expect("Loaded buffer that was just pushed does not exist.")
            }
        };
        let model_buffer = {
            let ref imb = self.instance_model_buffers[img_idx_usize];
            imb.load_data(&self.device, |target| {
                let stride = 16; // 16 floats = one 4x4 matrix
                for i in 0..mm.len().min(Self::MAX_INSTANCE_COUNT) {
                    target[i*stride..(i+1)*stride].copy_from_slice(&mm[i].mat().data);
                }
            })?;
            imb
        };
        if let Some(t) = mm[0].source.texture() {
            // load image
            let mut load = None;
            for li in self.loaded_images.iter() {
                if li.name == *t {
                    load = Some(li)
                }
            }
            let li = if let Some(l) = load { l } else {
                self.loaded_images.push(LoadedImage::new(
                    &self._adapter, &mut self.device, &mut self.command_pool, &mut self.queue_group.queues[0],
                    img::open(t).expect("Texture broken!").to_rgba(), t.clone(),
                )?);
                self.loaded_images.last().expect("Something that was just put into the vector is missing.")
            };
            // bind to descriptor set
            unsafe { self.device.write_descriptor_sets(vec![gfx_hal::pso::DescriptorSetWrite {
                set: &self.descriptor_sets[img_idx_usize], binding: 0, array_offset: 0,
                descriptors: Some(gfx_hal::pso::Descriptor::Image(
                    li.img_view_ref(),
                    Layout::ShaderReadOnlyOptimal
                )),
            }, gfx_hal::pso::DescriptorSetWrite {
                  set: &self.descriptor_sets[img_idx_usize], binding: 1, array_offset: 0,
                  descriptors: Some(gfx_hal::pso::Descriptor::Sampler(li.sampler_ref())),
            }]); }
        }
        // RECORD COMMANDS + BIND BUFFERS
        unsafe {
            let buffer = &mut self.command_buffers[img_idx_usize];
            const TRIANGLE_CLEAR: [ClearValue; 2] = [
                ClearValue::Color(ClearColor::Float([0f32, 0f32, 0f32, 1.0])),
                ClearValue::DepthStencil(ClearDepthStencil(1.0, 0)),
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
                let vert_buffer_ref: &B::Buffer = &vert_buffer.buffer_ref();
                let model_buffer_ref: &B::Buffer = &model_buffer.buffer_ref();
                let buffers: ArrayVec<[_; 2]> = [(vert_buffer_ref, 0), (model_buffer_ref, 0)].into();
                encoder.bind_vertex_buffers(0, buffers);
                encoder.bind_index_buffer(buffer::IndexBufferView {
                    buffer: index_buffer.buffer_ref(),
                    offset: 0,
                    index_type: Self::face_index_type(),
                });
                if mm[0].source.has_texture() {
                    encoder.bind_graphics_descriptor_sets(
                        &self.pipeline_layout, 0,
                        Some(&self.descriptor_sets[img_idx_usize]), &[],
                    );
                }
                let vp = cam.get_vp_mat();
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::VERTEX,
                    0, &vp.as_slice().iter().map(|f| (*f).to_bits()).collect::<Vec<u32>>()[..]
                );
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::FRAGMENT,
                    64, &Self::as_buffer(&color.into())
                );
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::FRAGMENT,
                    80, &[if mm[0].source.has_texture() { 1 } else { 0 }]
                );
                encoder.draw_indexed(
                    0..(mm[0].source.ff_flat_cnt() as u32),
                    0,
                    0..(mm.len().min(Self::MAX_INSTANCE_COUNT) as u32)
                );
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
                .map_err(|_| "Failed to present into the swapchain!")?;
        };
        Ok(())
    }
    fn draw_instanced_geom_no_depth<C: Into<[f32; 4]>>(&mut self, mm: Vec<geom::Model>, cam: geom::camera::Camera, color: C) -> Result<(), &'static str> {
        // FRAME PREP
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
        let (img_idx_u32, img_idx_usize) = unsafe {
            let (image_index, _optimality) = self.swapchain.acquire_image(core::u64::MAX, Some(image_available), None)
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
        // LOAD ALL NEEDED DATA
        let vert_buffer = {
            let ref vb = self.alloc_buffers[0];
            vb.load_data_from_slice(&mut self.device, &mm[0].vv_as_bytes(), 0)?;
            vb
        };
        let index_buffer = {
            let mut found_buffer = None;
            // TODO use a pointer map for O(1)ish look up later
            for b in self.loaded_buffers.iter() {
                if b.matches_source(&mm[0].source) {
                    found_buffer = Some(b);
                    break;
                }
            }
            if let Some(b) = found_buffer { b } else {
                self.loaded_buffers.push(LoadedBuffer::new(
                    &self._adapter, &mut self.device,
                    None,
                    (&**mm[0].source).ff_byte_cnt() as u64, buffer::Usage::INDEX,
                    &mm[0].ff_as_bytes(), mm[0].source.clone()
                )?);
                self.loaded_buffers.last().expect("Loaded buffer that was just pushed does not exist.")
            }
        };
        let model_buffer = {
            let ref imb = self.instance_model_buffers[img_idx_usize];
            imb.load_data(&self.device, |target| {
                let stride = 16; // 16 floats = one 4x4 matrix
                for i in 0..mm.len().min(Self::MAX_INSTANCE_COUNT) {
                    target[i*stride..(i+1)*stride].copy_from_slice(&mm[i].mat().data);
                }
            })?;
            imb
        };
        if let Some(t) = mm[0].source.texture() {
            // load image
            let mut load = None;
            for li in self.loaded_images.iter() {
                if li.name == *t {
                    load = Some(li)
                }
            }
            let li = if let Some(l) = load { l } else {
                self.loaded_images.push(LoadedImage::new(
                    &self._adapter, &mut self.device, &mut self.command_pool, &mut self.queue_group.queues[0],
                    img::open(t).expect("Texture broken!").to_rgba(), t.clone(),
                )?);
                self.loaded_images.last().expect("Something that was just put into the vector is missing.")
            };
            // bind to descriptor set
            unsafe { self.device.write_descriptor_sets(vec![gfx_hal::pso::DescriptorSetWrite {
                set: &self.descriptor_sets[img_idx_usize], binding: 0, array_offset: 0,
                descriptors: Some(gfx_hal::pso::Descriptor::Image(
                    li.img_view_ref(),
                    Layout::ShaderReadOnlyOptimal
                )),
            }, gfx_hal::pso::DescriptorSetWrite {
                  set: &self.descriptor_sets[img_idx_usize], binding: 1, array_offset: 0,
                  descriptors: Some(gfx_hal::pso::Descriptor::Sampler(li.sampler_ref())),
            }]); }
        }
        // RECORD COMMANDS + BIND BUFFERS
        unsafe {
            let buffer = &mut self.command_buffers[img_idx_usize];
            const TRIANGLE_CLEAR: [ClearValue; 1] = [
                ClearValue::Color(ClearColor::Float([0f32, 0f32, 0f32, 1.0])),
            ];
            buffer.begin(false);
            {
                let mut encoder = buffer.begin_render_pass_inline(
                    &self.render_pass_no_depth,
                    &self.framebuffers_no_depth[img_idx_usize],
                    self.render_area,
                    TRIANGLE_CLEAR.iter(),
                );
                encoder.bind_graphics_pipeline(&self.graphics_pipeline_no_depth);
                let vert_buffer_ref: &B::Buffer = &vert_buffer.buffer_ref();
                let model_buffer_ref: &B::Buffer = &model_buffer.buffer_ref();
                let buffers: ArrayVec<[_; 2]> = [(vert_buffer_ref, 0), (model_buffer_ref, 0)].into();
                encoder.bind_vertex_buffers(0, buffers);
                encoder.bind_index_buffer(buffer::IndexBufferView {
                    buffer: index_buffer.buffer_ref(),
                    offset: 0,
                    index_type: Self::face_index_type(),
                });
                if mm[0].source.has_texture() {
                    encoder.bind_graphics_descriptor_sets(
                        &self.pipeline_layout, 0,
                        Some(&self.descriptor_sets[img_idx_usize]), &[],
                    );
                }
                let vp = cam.get_vp_mat();
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::VERTEX,
                    0, &vp.as_slice().iter().map(|f| (*f).to_bits()).collect::<Vec<u32>>()[..]
                );
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::FRAGMENT,
                    64, &Self::as_buffer(&color.into())
                );
                encoder.push_graphics_constants(
                    &self.pipeline_layout, ShaderStageFlags::FRAGMENT,
                    80, &[if mm[0].source.has_texture() { 1 } else { 0 }]
                );
                encoder.draw_indexed(
                    0..(mm[0].source.ff_flat_cnt() as u32),
                    0,
                    0..(mm.len().min(Self::MAX_INSTANCE_COUNT) as u32)
                );
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
            let _optimality = self.swapchain.present(the_command_queue, img_idx_u32, present_wait_semaphores)
                .map_err(|_| "Failed to present into the swapchain!")?;
        };
        Ok(())
    }
    fn draw_geom_direct(&mut self, m: geom::Model, cam: geom::camera::Camera) -> Result<(), &'static str> {
        unimplemented!();
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
            RenderReq::Draw(m, cam, Color(c)) => r.draw_instanced_geom(vec![m], cam, c),
            RenderReq::DrawDirect(m, cam, _) => r.draw_geom_direct(m, cam),
            RenderReq::DrawGroup(mm, cam, Color(c)) => r.draw_instanced_geom(mm, cam, c),
            RenderReq::DrawGroupWithSetting(mm, cam, Color(c), settings) => {
                if settings.should_use_depth {
                    r.draw_instanced_geom(mm, cam, c)
                } else {
                    r.draw_instanced_geom_no_depth(mm, cam, c)
                }
            },
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

            self.descriptor_sets.drain(..);
            // self.descriptor_pool.free_sets(self.descriptor_sets.drain(..)); // implicitly done
            self.device.destroy_descriptor_pool(ManuallyDrop::into_inner(read(&mut self.descriptor_pool)));

            self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&mut self.graphics_pipeline)));
            self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&mut self.graphics_pipeline_no_depth)));
            self.device.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&mut self.pipeline_layout)));
            for dsl in self.descriptor_set_layouts.drain(..) {
                self.device.destroy_descriptor_set_layout(dsl);
            }

            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb)
            }
            for fb in self.framebuffers_no_depth.drain(..) {
                self.device.destroy_framebuffer(fb)
            }
            for db in self.depth_buffers.drain(..) {
                db.free(&self.device);
            }
            for iv in self.image_views.drain(..) {
                self.device.destroy_image_view(iv)
            }

            for b in self.loaded_images.drain(..) {
                b.free(&self.device);
            }
            for imb in self.instance_model_buffers.drain(..) {
                imb.free(&self.device);
            }
            for b in self.alloc_buffers.drain(..) {
                b.free(&self.device);
            }
            for b in self.loaded_buffers.drain(..) {
                b.free(&self.device);
            }

            // The CommandPool must also be unwrapped into a RawCommandPool,
            // so there's an extra `into_raw` call here.
            self.device.destroy_command_pool(ManuallyDrop::into_inner(read(&mut self.command_pool)).into_raw());
            self.device.destroy_render_pass(
                ManuallyDrop::into_inner(read(&mut self.render_pass))
            );
            self.device.destroy_render_pass(
                ManuallyDrop::into_inner(read(&mut self.render_pass_no_depth))
            );
            self.device.destroy_swapchain(
                ManuallyDrop::into_inner(read(&mut self.swapchain))
            );
            ManuallyDrop::drop(&mut self.graphics_pipeline);
            ManuallyDrop::drop(&mut self.graphics_pipeline_no_depth);
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
                    info!("Recreating renderer!");
                    drop(r);
                    r = f(&*w).expect("Fuck. Couldn't create a thingy-thing after the first time.");
                    info!("Renderer recreated!");
                } else {
                    match Renderer::handle_req(&mut r, req) {
                        Ok(_) => (),
                        Err(s) => {
                            info!("Recreating renderer!");
                            error!("Error ({:?}) while handling request. Attempting recovery...", s);
                            drop(r);
                            r = f(&*w).expect("Fuck. Couldn't create a thingy-thing after the first time.");
                            info!("Renderer recreated!");
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
pub type FinishResult = kt::FinishResult<()>;
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

