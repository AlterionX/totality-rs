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
use std::{
    time::SystemTime,
    result::Result,
    sync::{Arc,Mutex},
    mem::ManuallyDrop,
    ptr::read,
};
use na::{Vector3,Vector4};
use winit::Window;
use log::{info};

pub struct Renderer {
    current_frame: usize,
    max_frames_in_flight: usize,
    in_flight_fences: Vec<<back::Backend as Backend>::Fence>,
    render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    command_buffers: Vec<CommandBuffer<back::Backend, Graphics, MultiShot, Primary>>,
    command_pool: ManuallyDrop<CommandPool<back::Backend, Graphics>>,
    framebuffers: Vec<<back::Backend as Backend>::Framebuffer>,
    image_views: Vec<(<back::Backend as Backend>::ImageView)>,
    render_pass: ManuallyDrop<<back::Backend as Backend>::RenderPass>,
    render_area: Rect,
    queue_group: ManuallyDrop<QueueGroup<back::Backend, Graphics>>,
    swapchain: ManuallyDrop<<back::Backend as Backend>::Swapchain>,
    device: ManuallyDrop<back::Device>,
    _adapter: Adapter<back::Backend>,
    _surface: <back::Backend as Backend>::Surface,
    _instance: ManuallyDrop<back::Instance>,
}

impl Renderer {
    pub fn new(w: &Window) -> Result<Self, &str> {
        let inst = back::Instance::create("Tracer", 1);
        let mut surf = inst.create_surface(w);
        let adapter = hal::Instance::enumerate_adapters(&inst)
            .into_iter()
            .find(|a| {
                a.queue_families.iter()
                    .any(|qf| qf.supports_graphics() && surf.supports_queue_family(qf))
             })
             .ok_or("Couldn't find a graphical Adapter!")?;
        let (device, queue_group) = {
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
            let extent = caps.extents.end;
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
            let mut image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore> = vec![];
            let mut render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore> = vec![];
            let mut in_flight_fences: Vec<<back::Backend as Backend>::Fence> = vec![];
            for _ in 0..max_frames_in_flight {
                in_flight_fences.push(device.create_fence(true).map_err(|_| "Could not create a fence!")?);
                image_available_semaphores.push(device.create_semaphore().map_err(|_| "Could not create a semaphore!")?);
                render_finished_semaphores.push(device.create_semaphore().map_err(|_| "Could not create a semaphore!")?);
            }
            (image_available_semaphores, render_finished_semaphores, in_flight_fences)
        };
        let render_pass = {
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
        let framebuffers: Vec<<back::Backend as Backend>::Framebuffer> = {
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
    pub fn draw_empty_scene(&mut self) -> Result<(), &str> {
        let since_epoch = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => (((n.as_nanos() % 1_000_000_000u128) as f64) / 1_000_000_000f64) as f32,
            Err(_) => 1f32
        };
        let col = Vector4::repeat(since_epoch);
        self.clear_color(col)
    }
    pub fn clear_color<C>(&mut self, color: C) -> Result<(), &str> where C: Into<[f32; 4]> {
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
}
impl Drop for Renderer {
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
            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb)
            }
            for iv in self.image_views.drain(..) {
                self.device.destroy_image_view(iv)
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
            ManuallyDrop::drop(&mut self.queue_group);
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self._instance);
        }
    }
}

