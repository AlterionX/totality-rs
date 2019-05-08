use super::{
    hal::{
        Backend, Device, Adapter, PhysicalDevice,
        memory::{Requirements, Properties, Barrier},
        adapter::MemoryTypeId,
        queue::{CommandQueue,capability::{Capability, Supports, Transfer}},
        pool::CommandPool,
        buffer::Usage,
        format::{Format, Swizzle, Aspects},
        image::{self, ViewKind, SubresourceRange, Layout},
        pso::PipelineStage,
        command::BufferImageCopy,
        window::Extent2D,
    },
    img,
    buffers::AllocatedBuffer,
};
use std::{
    mem::{ManuallyDrop, size_of},
    marker::PhantomData,
};

#[allow(dead_code)]
use log::{error, warn, info, debug, trace};

// Parts for a depth buffer image
pub struct DepthImage<B: Backend> {
  pub image: ManuallyDrop<B::Image>,
  pub requirements: Requirements,
  pub memory: ManuallyDrop<B::Memory>,
  pub image_view: ManuallyDrop<B::ImageView>,
  pub phantom: PhantomData<B::Device>,
}
impl<B: Backend<Device=D>, D: Device<B>> DepthImage<B> {
  pub fn new(adapter: &Adapter<B>, device: &D, extent: Extent2D) -> Result<Self, &'static str> {
    unsafe {
      let mut the_image = device
        .create_image(
          gfx_hal::image::Kind::D2(extent.width, extent.height, 1, 1),
          1,
          Format::D32Float,
          gfx_hal::image::Tiling::Optimal,
          gfx_hal::image::Usage::DEPTH_STENCIL_ATTACHMENT,
          gfx_hal::image::ViewCapabilities::empty(),
        )
        .map_err(|_| "Couldn't crate the image!")?;
      let requirements = device.get_image_requirements(&the_image);
      let memory_type_id = adapter
        .physical_device
        .memory_properties()
        .memory_types
        .iter()
        .enumerate()
        .find(|&(id, memory_type)| {
          // BIG NOTE: THIS IS DEVICE LOCAL NOT CPU VISIBLE
          requirements.type_mask & (1 << id) != 0
            && memory_type.properties.contains(Properties::DEVICE_LOCAL)
        })
        .map(|(id, _)| MemoryTypeId(id))
        .ok_or("Couldn't find a memory type to support the image!")?;
      let memory = device
        .allocate_memory(memory_type_id, requirements.size)
        .map_err(|_| "Couldn't allocate image memory!")?;
      device
        .bind_image_memory(&memory, 0, &mut the_image)
        .map_err(|_| "Couldn't bind the image memory!")?;
      let image_view = device
        .create_image_view(
          &the_image,
          gfx_hal::image::ViewKind::D2,
          Format::D32Float,
          gfx_hal::format::Swizzle::NO,
          SubresourceRange {
            aspects: Aspects::DEPTH,
            levels: 0..1,
            layers: 0..1,
          },
        )
        .map_err(|_| "Couldn't create the image view!")?;
      Ok(Self {
        image: ManuallyDrop::new(the_image),
        requirements,
        memory: ManuallyDrop::new(memory),
        image_view: ManuallyDrop::new(image_view),
        phantom: PhantomData,
      })
    }
  }

    pub fn img_view_ref(&self) -> &B::ImageView { &self.image_view }
    pub unsafe fn free(&self, device: &D) {
        use core::ptr::read;
        device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
        device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
        device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));
    }
}

pub struct LoadedImage<B: Backend> {
    pub name: String,
    image: ManuallyDrop<B::Image>,
    requirements: Requirements,
    memory: ManuallyDrop<B::Memory>,
    image_view: ManuallyDrop<B::ImageView>,
    sampler: ManuallyDrop<B::Sampler>,
    phantom: PhantomData<B::Device>,
}
impl <B: Backend> LoadedImage<B> {
    pub fn new<C: Capability + Supports<Transfer>>(
        adapter: &Adapter<B>, device: &mut B::Device, command_pool: &mut CommandPool<B, C>,
        command_queue: &mut CommandQueue<B, C>, img: img::RgbaImage,
        name: String
    ) -> Result<Self, &'static str> {
        unsafe {
            // 0. First we compute some memory related values.
            let pixel_size = size_of::<img::Rgba<u8>>();
            let row_size = pixel_size * (img.width() as usize);
            let limits = adapter.physical_device.limits();
            let row_alignment_mask = limits.min_buffer_copy_pitch_alignment as u32 - 1;
            let row_pitch = ((row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
            debug_assert!(row_pitch as usize >= row_size);
            // 1. make a staging buffer with enough memory for the image, and a
            //    transfer_src usage
            let required_bytes = row_pitch * img.height() as usize;
            let mut staging_buffer = AllocatedBuffer::new(
                &adapter, device, None,
                required_bytes as u64,
                Usage::TRANSFER_SRC
            )?;
            // 2. use mapping writer to put the image data into that buffer
            staging_buffer.load_data(device, |target| {
                for y in 0..img.height() as usize {
                    let row = &(*img)[y * row_size..(y + 1) * row_size];
                    let dest_base = y * row_pitch;
                    target[dest_base..dest_base + row.len()].copy_from_slice(row);
                }
            });
            // 3. Make an image with transfer_dst and SAMPLED usage
            let mut the_image = device.create_image(
                  gfx_hal::image::Kind::D2(img.width(), img.height(), 1, 1),
                  1,
                  Format::Rgba8Srgb,
                  image::Tiling::Optimal,
                  image::Usage::TRANSFER_DST | gfx_hal::image::Usage::SAMPLED,
                  image::ViewCapabilities::empty(),
            ).map_err(|_| "Couldn't create the image!")?;
            // 4. allocate memory for the image and bind it
            let requirements = device.get_image_requirements(&the_image);
            let memory_type_id = adapter.physical_device.memory_properties()
                .memory_types.iter().enumerate()
                .find(|&(id, memory_type)| {
                    // BIG NOTE: THIS IS DEVICE LOCAL NOT CPU VISIBLE
                    requirements.type_mask & (1 << id) != 0
                        && memory_type.properties.contains(Properties::DEVICE_LOCAL)
                }).map(|(id, _)| MemoryTypeId(id))
                .ok_or("Couldn't find a memory type to support the image!")?;
            let memory = device.allocate_memory(memory_type_id, requirements.size)
                .map_err(|_| "Couldn't allocate image memory!")?;
            device.bind_image_memory(&memory, 0, &mut the_image)
                .map_err(|_| "Couldn't bind the image memory!")?;
            // 5. create image view and sampler
            let image_view = device.create_image_view(
                &the_image,
                ViewKind::D2,
                Format::Rgba8Srgb,
                Swizzle::NO,
                SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            ).map_err(|_| "Couldn't create the image view!")?;
            let sampler = device.create_sampler(gfx_hal::image::SamplerInfo::new(
                gfx_hal::image::Filter::Nearest, gfx_hal::image::WrapMode::Tile,
            )).map_err(|_| "Couldn't create the sampler!")?;
            // 6. create a command buffer
            let mut cmd_buffer = command_pool.acquire_command_buffer::<gfx_hal::command::OneShot>();
            cmd_buffer.begin();
            // 7. Use a pipeline barrier to transition the image from empty/undefined
            //    to TRANSFER_WRITE/TransferDstOptimal
            let image_barrier = Barrier::Image {
                states: (gfx_hal::image::Access::empty(), Layout::Undefined)
                    ..(
                      gfx_hal::image::Access::TRANSFER_WRITE,
                      Layout::TransferDstOptimal,
                    ),
                target: &the_image,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            };
            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                gfx_hal::memory::Dependencies::empty(),
                &[image_barrier],
            );
            // 8. perform copy from staging buffer to image
            cmd_buffer.copy_buffer_to_image(
                staging_buffer.buffer_ref(),
                &the_image,
                Layout::TransferDstOptimal,
                &[BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: (row_pitch / pixel_size) as u32,
                    buffer_height: img.height(),
                    image_layers: gfx_hal::image::SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: gfx_hal::image::Offset { x: 0, y: 0, z: 0 },
                    image_extent: gfx_hal::image::Extent {
                        width: img.width(),
                        height: img.height(),
                        depth: 1,
                    },
                }],
            );
            // 9. use pipeline barrier to transition the image to SHADER_READ access/
            //    ShaderReadOnlyOptimal layout
            let image_barrier = gfx_hal::memory::Barrier::Image {
                states: (
                    gfx_hal::image::Access::TRANSFER_WRITE,
                    Layout::TransferDstOptimal,
                )..(
                    gfx_hal::image::Access::SHADER_READ,
                    Layout::ShaderReadOnlyOptimal,
                ),
                target: &the_image,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            };
            cmd_buffer.pipeline_barrier(
                PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                gfx_hal::memory::Dependencies::empty(),
                &[image_barrier],
            );
            // 10. Submit the cmd buffer to queue and wait for it
            cmd_buffer.finish();
            let upload_fence = device
                .create_fence(false)
                .map_err(|_| "Couldn't create an upload fence!")?;
            command_queue.submit_nosemaphores(Some(&cmd_buffer), Some(&upload_fence));
            device
                .wait_for_fence(&upload_fence, core::u64::MAX)
                .map_err(|_| "Couldn't wait for the fence!")?;
            device.destroy_fence(upload_fence);
            // 11. Destroy the staging bundle and one shot buffer now that we're done
            staging_buffer.free(device);
            command_pool.free(Some(cmd_buffer));
            Ok(LoadedImage {
                name: name,
                image: ManuallyDrop::new(the_image),
                requirements: requirements,
                memory: ManuallyDrop::new(memory),
                image_view: ManuallyDrop::new(image_view),
                sampler: ManuallyDrop::new(sampler),
                phantom: PhantomData,
            })
        }
    }
    pub fn img_view_ref(&self) -> &B::ImageView { &self.image_view }
    pub fn sampler_ref(&self) -> &B::Sampler { &self.sampler }
    pub unsafe fn free(&self, device: &B::Device) {
        use core::ptr::read;
        device.destroy_sampler(ManuallyDrop::into_inner(read(&self.sampler)));
        device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
        device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
        device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));
    }
}
