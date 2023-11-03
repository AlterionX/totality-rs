pub mod task;
mod shaders;

use std::{sync::Arc, collections::{HashMap, hash_map::Entry}, time::{Instant, Duration}};

use img::{ImageDecoder, codecs::png::{PngDecoder, PngReader}, ImageFormat};
use raw_window_handle::HandleError;
use tap::{TapFallible, TapOptional};
use task::RenderTask;
use thiserror::Error;

use vulkano::{
    VulkanLibrary,
    VulkanError,
    instance::{
        Instance,
        InstanceCreateInfo,
    },
    LoadingError,
    Version,
    swapchain::{
        Surface,
        Swapchain,
        SwapchainCreateInfo,
        CompositeAlpha,
        self,
        SwapchainPresentInfo,
    },
    Validated,
    device::{
        physical::{
            PhysicalDeviceType,
            PhysicalDevice
        },
        Device,
        QueueFlags,
        DeviceCreateInfo,
        QueueCreateInfo,
        Queue,
        DeviceExtensions, Features,
    },
    memory::allocator::{
        StandardMemoryAllocator,
        AllocationCreateInfo,
        MemoryTypeFilter,
    },
    buffer::{
        Buffer,
        BufferCreateInfo,
        BufferUsage,
        Subbuffer,
        AllocateBufferError,
        IndexBuffer,
    },
    command_buffer::{
        allocator::{
            StandardCommandBufferAllocator,
            StandardCommandBufferAllocatorCreateInfo, CommandBufferAlloc,
        },
        AutoCommandBufferBuilder,
        CommandBufferUsage,
        RenderPassBeginInfo,
        SubpassBeginInfo,
        SubpassContents,
        SubpassEndInfo,
        ClearAttachment,
        ClearRect, CopyBufferToImageInfo, sys::{UnsafeCommandBuffer, UnsafeCommandBufferBuilder, CommandBufferBeginInfo}, CommandBufferLevel, CommandBufferInheritanceInfo, SecondaryCommandBufferAbstract,
    },
    image::{
        ImageUsage,
        Image,
        view::{ImageView, ImageViewCreateInfo},
        SampleCount,
        ImageCreateInfo,
        ImageType, ImageCreateFlags, ImageLayout, ImageTiling, ImageSubresourceRange, sampler::{Sampler, SamplerCreateInfo},
    },
    format::{
        Format,
        ClearValue,
    },
    render_pass::{
        RenderPass,
        Framebuffer,
        Subpass,
    },
    shader::{
        ShaderModule,
        ShaderStages,
    },
    sync::{
        HostAccessError,
        GpuFuture, ImageMemoryBarrier, DependencyInfo, DependencyFlags,
    },
    pipeline::{
        GraphicsPipeline,
        graphics::{
            GraphicsPipelineCreateInfo,
            rasterization::{
                RasterizationState,
                CullMode,
            },
            input_assembly::{
                InputAssemblyState,
                PrimitiveTopology,
            },
            vertex_input::{
                VertexInputState,
                VertexInputBindingDescription,
                VertexInputRate,
                VertexInputAttributeDescription,
            },
            viewport::{
                ViewportState,
                Viewport,
                Scissor,
            },
            multisample::MultisampleState,
            subpass::PipelineSubpassType,
            color_blend::{
                ColorBlendState,
                ColorBlendAttachmentState,
            },
            depth_stencil::{
                DepthStencilState,
                DepthState,
            },
        },
        PipelineLayout,
        layout::{
            PipelineLayoutCreateInfo,
            PipelineLayoutCreateFlags,
            PushConstantRange,
        },
        PipelineShaderStageCreateInfo, PipelineBindPoint, Pipeline,
    },
    descriptor_set::{
        pool::{
            DescriptorPool,
            DescriptorPoolCreateInfo,
            DescriptorPoolCreateFlags,
        },
        layout::{
            DescriptorType,
            DescriptorSetLayout,
            DescriptorSetLayoutCreateInfo,
            DescriptorSetLayoutCreateFlags,
            DescriptorSetLayoutBinding,
        },
        allocator::{
            StandardDescriptorSetAllocator,
            StandardDescriptorSetAllocatorCreateInfo, DescriptorSetAllocator, StandardDescriptorSetAlloc,
        },
        PersistentDescriptorSet,
        WriteDescriptorSet, DescriptorSet, DescriptorSetWithOffsets,
    },
};
use winit::{window::{WindowId, Window}, dpi::PhysicalSize};

#[derive(Debug, Default)]
pub struct RendererPreferences {
    pub preferred_physical_device: Option<String>,
    pub preferred_physical_device_type: Option<PhysicalDeviceType>,
}

impl RendererPreferences {
    // This is a "generally better" heuristic. (Higher is better.)
    fn score_physical_device(&self, physical_device: &PhysicalDevice) -> usize {
        let props = physical_device.properties();

        let name_score = if let Some(ref name) = self.preferred_physical_device {
            if props.device_name == *name {
                1
            } else {
                0
            }
        } else {
            0
        };

        const PDT_PREF_SCALING: usize = 2;
        let pdt_pref_score = if let Some(ref pdt) = self.preferred_physical_device_type {
            if props.device_type == *pdt {
                1
            } else {
                0
            }
        } else {
            0
        };

        // We'll just use an array here -- a match doesn't really help.
        // Ordered from least preferred to most. Later entries have higher indices so they're more
        // preferred.
        const PDT_ORD_ARR: [PhysicalDeviceType; 5] = [
            PhysicalDeviceType::Other,
            // I don't actually know if CPU is preferred less than or over the virtual GPU.
            PhysicalDeviceType::Cpu,
            PhysicalDeviceType::VirtualGpu,
            PhysicalDeviceType::IntegratedGpu,
            PhysicalDeviceType::DiscreteGpu,
        ];
        let pdt_ord_score = 'a: {
            for (idx, pdt) in PDT_ORD_ARR.iter().enumerate() {
                if *pdt == props.device_type {
                    // Add one to offset away from the zero value, which represents "I couldn't
                    // find it"
                    break 'a idx + 1;
                }
            }
            0
        };

        (name_score << 1 + pdt_pref_score) * (PDT_ORD_ARR.len() + 1) + pdt_ord_score
    }
}

pub struct LoadedModelHandles {
    texture_image: Option<Arc<Image>>,
    texture_descriptor_set: Option<Arc<PersistentDescriptorSet>>,
    vertex_offset: i32,
    index_offset: u32,
}

#[derive(Debug, Error)]
pub enum RendererInitializationError {
    #[error("{0}")]
    Library(#[from] LoadingError),
    #[error("instance initialization: {0}")]
    Instance(Validated<VulkanError>),
    #[error("physical device enumeration: {0}")]
    Physical(VulkanError),
    #[error("no physical device -- no software fallback")]
    NoPhysicalDevice,
    #[error("no valid physical device -- no physical device with graphics queue family")]
    PhysicalDeviceMissingGraphicsCapabilities,
    #[error("failed to create queue: {0}")]
    QueueCreationFailed(Validated<VulkanError>),
    #[error("failed to create buffer: {0}")]
    BufferCreationFailed(Validated<AllocateBufferError>),
    #[error("no queue for device")]
    NoCommandQueue,
    #[error("failed to create commmand buffer: {0}")]
    CommandBufferCreationFailed(Validated<VulkanError>),
    #[error("failed to compile shader {0}: {1}")]
    ShaderLoadFail(&'static str, Validated<VulkanError>),
    #[error("surface extension enumeration: {0}")]
    SurfaceCheck(#[from] HandleError),
}

pub struct Renderer {
    vulkan: Arc<Instance>,

    vertex_shader: Arc<ShaderModule>,
    geometry_shader: Arc<ShaderModule>,
    fragment_shader: Arc<ShaderModule>,

    ordered_physical_devices: Vec<Arc<PhysicalDevice>>,
    selected_physical_device_idx: usize,
    selected_device: Arc<Device>,
    selected_device_queues: Vec<Arc<Queue>>,

    device_memory_alloc: Arc<StandardMemoryAllocator>,

    vertex_buffer: Subbuffer<[u8]>,
    face_buffer: Subbuffer<[u8]>,
    uniform_counts_buffer: Subbuffer<[u8]>,
    uniform_per_mesh_buffer: Subbuffer<[u8]>,
    uniform_light_buffer: Subbuffer<[u8]>,
    uniform_material_buffer: Subbuffer<[u8]>,
    constants_buffer: Subbuffer<[u8]>,
    staging_texture_buffer: Subbuffer<[u8]>,

    command_buffer_alloc: Arc<StandardCommandBufferAllocator>,

    pipeline_layout: Arc<PipelineLayout>,

    descriptor_set_layout: Arc<DescriptorSetLayout>,
    texture_descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_pool: DescriptorPool,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    descriptor_set: Arc<PersistentDescriptorSet>,
    empty_texture_descriptor_set: Arc<PersistentDescriptorSet>,
    texture_sampler: Arc<Sampler>,

    // TODO: better map for window ids?
    windowed_swapchain: HashMap<WindowId, RendererWindowSwapchain>,

    // Mesh id to (vertex, index) buffer offsets.
    loaded_models: HashMap<u64, LoadedModelHandles>,
    vertex_free_byte_start: u64,
    index_free_byte_start: u64,
    vertex_free_start: i32,
    index_free_start: u32,
}

impl Renderer {
    pub fn init(application_name: Option<String>, application_version: Option<Version>, windowing: Arc<Window>, pref: &RendererPreferences) -> Result<Self, RendererInitializationError> {
        let vulkan_library = VulkanLibrary::new()
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED primary_source=vulkan_lib error=missing_error {e}"))?;
        let required_extensions = Surface::required_extensions(&windowing)
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED primary_source=surface_check {e}"))?;
        let vulkan = Instance::new(
            vulkan_library,
            InstanceCreateInfo {
                enabled_extensions: required_extensions,
                engine_name: Some("totality".to_owned()),
                engine_version: Version::default(),
                application_name,
                application_version: application_version.unwrap_or_default(),
                // enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_owned(), "VK_LAYER_LUNARG_api_dump".to_owned()],
                ..Default::default()
            }
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=driver error=instance {e}"))
            .map_err(RendererInitializationError::Instance)?;

        // Find best (most performant or preferred) valid device.
        let ordered_physical_devices = {
            let iter = vulkan.enumerate_physical_devices()
                .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=physical_devices error=enumeration_failure {e}"))
                .map_err(RendererInitializationError::Physical)?;
            let minimum_device_extensions = DeviceExtensions {
                khr_swapchain: true,
                ..DeviceExtensions::empty()
            };
            let mut devices = iter.filter(|pd| pd.supported_extensions().contains(&minimum_device_extensions)).collect::<Vec<_>>();
            devices.sort_by_cached_key(|pd| pref.score_physical_device(pd));
            devices
        };

        // We need something that can *actually* render something.
        let (selected_physical_device_idx, selected_queue_family_idx) = 'queue_family: {
            // Assume there's at least one physical device and that the first device is valid.
            let physical = ordered_physical_devices.first()
                .tap_none(|| log::error!("TOTALITY-RENDERER-INIT-FAILED source=physical_devices error=no_device"))
                .ok_or_else(|| RendererInitializationError::NoPhysicalDevice)?;
            let queue_props = physical.queue_family_properties();
            // Just pick the first valid one for now, we'll come back.
            // TODO Be smarter about selecting a queue family, or be dynamic about it.
            // TODO disqualify queues that don't support a surface
            for (idx, family) in queue_props.iter().enumerate() {
                if family.queue_flags.contains(QueueFlags::GRAPHICS) {
                    let idx_u32 = idx.try_into().expect("number of queues is small enough to fit into a u32");
                    break 'queue_family (0, idx_u32);
                }
            }
            return Err(RendererInitializationError::PhysicalDeviceMissingGraphicsCapabilities);
        };

        let (selected_device, selected_device_queues) = {
            let required_extensions = DeviceExtensions {
                khr_swapchain: true,
                khr_push_descriptor: true,
                ..DeviceExtensions::empty()
            };
            let (device, queues_iter) = Device::new(
                Arc::clone(&ordered_physical_devices[0]),
                DeviceCreateInfo {
                    enabled_features: Features {
                        geometry_shader: true,
                        ..Features::empty()
                    },
                    queue_create_infos: vec![QueueCreateInfo {
                        queue_family_index: selected_queue_family_idx,
                        ..Default::default()
                    }],
                    enabled_extensions: required_extensions,
                    ..Default::default()
                },
            )
                .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=device_queue error=failed_creation {e}"))
                .map_err(RendererInitializationError::QueueCreationFailed)?;
            let queues: Vec<_> = queues_iter.collect();
            if queues.is_empty() {
                return Err(RendererInitializationError::NoCommandQueue);
            }
            (device, queues)
        };

        // TODO Reevaluate if we can adjust the memory allocator.
        let device_memory_alloc = Arc::new(StandardMemoryAllocator::new_default(Arc::clone(&selected_device)));
        // TODO Can we get rid of this?
        let dyn_device_memory_alloc = Arc::clone(&device_memory_alloc) as _;

        // Let's just allocate a giant chunk for vertices.
        let vertex_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: BufferCreateFlags::default(),
                // sharing: (),
                // size: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This should be fine. A vertex is 3*32 bits -- 12 bytes.
            // Let's just assume each scene will have < 5_000_000 vertices. So let's just allocate
            // 60 MB. Then add an extra vector.
            60 * 1024 * 1024
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=vertex_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?
        ;
        // And then for triangles.
        let face_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: BufferCreateFlags::default(),
                // sharing: (),
                // size: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This should be fine. A face is 3*32 bits -- 12 bytes.
            // Let's just assume each scene will have < 5_000_000 faces. So let's just allocate
            // 60 MB. Then add an extra vector.
            60 * 1024 * 1024
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=face_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // One for instanced model sets.
        let uniform_counts_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: (),
                // sharing: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This allows for approximately 1024 instances.
            // Assuming a 4x4 32bit ...
            16,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=count_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // One for instanced model sets.
        let uniform_per_mesh_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: (),
                // sharing: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This allows for approximately 1024 instances.
            // Assuming a 4x4 32bit ...
            64 * 1024,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=matrix_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // A light buffer...
        let uniform_light_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: (),
                // sharing: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This allows for approximately 500 instances.
            // Assuming a 4x4 32bit float matrix for orientation, positioning, and scaling -- 16 *
            // 32 / 8 = 2^6 bytes per array. 2^6 * 500 = 32000. Let's just use ~50KB.
            96 * 1024,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=matrix_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // More for materials!
        let uniform_material_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: (),
                // sharing: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            // This allows for approximately 500 instances.
            // Assuming a 4x4 32bit float matrix for orientation, positioning, and scaling -- 16 *
            // 32 / 8 = 2^6 bytes per array. 2^6 * 500 = 32000. Let's just use ~50KB.
            16 * 1024,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=matrix_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // And another for textures.
        let staging_texture_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_TEXEL_BUFFER | BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC,
                // flags: (),
                // sharing: (),
                // size: (),
                // usage: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            50 * 1024 * 1024
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=staging_texture_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // And a last chunk for constants.
        let constants_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
                // flags: (),
                // sharing: (),
                // size: (),
                // external_memory_handle_types: (),
                ..Default::default()
            },
            AllocationCreateInfo {
                // memory_type_bits: (),
                // allocate_preference: (),
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            50 * 1024 * 1024
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=constants_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;

        let command_buffer_alloc = Arc::new(StandardCommandBufferAllocator::new(Arc::clone(&selected_device), StandardCommandBufferAllocatorCreateInfo::default()));

        let vertex_shader = shaders::basic_vert::load(Arc::clone(&selected_device))
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=shader shader=basic_vert {e}"))
            .map_err(|e| RendererInitializationError::ShaderLoadFail("basic_vert", e))?;
        let geometry_shader = shaders::basic_geom::load(Arc::clone(&selected_device))
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=shader shader=basic_geom {e}"))
            .map_err(|e| RendererInitializationError::ShaderLoadFail("basic_geom", e))?;
        let fragment_shader = shaders::basic_frag::load(Arc::clone(&selected_device))
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=shader shader=basic_frag {e}"))
            .map_err(|e| RendererInitializationError::ShaderLoadFail("basic_frag", e))?;

        let descriptor_set_layout = DescriptorSetLayout::new(Arc::clone(&selected_device), DescriptorSetLayoutCreateInfo {
            flags: DescriptorSetLayoutCreateFlags::empty(),
            bindings: [
                (0, DescriptorSetLayoutBinding {
                    descriptor_count: 1,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
                (1, DescriptorSetLayoutBinding {
                    descriptor_count: 1024,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
                (2, DescriptorSetLayoutBinding {
                    descriptor_count: 1024,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
                (3, DescriptorSetLayoutBinding {
                    descriptor_count: 1024,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
            ].into_iter().collect(),
            ..DescriptorSetLayoutCreateInfo::default()
        }).unwrap();
        let texture_descriptor_set_layout = DescriptorSetLayout::new(Arc::clone(&selected_device), DescriptorSetLayoutCreateInfo {
            flags: DescriptorSetLayoutCreateFlags::empty(),
            bindings: [
                (0, DescriptorSetLayoutBinding {
                    descriptor_count: 1,
                    stages: ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::SampledImage)
                }),
                (1, DescriptorSetLayoutBinding {
                    descriptor_count: 1,
                    stages: ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::Sampler)
                }),
            ].into_iter().collect(),
            ..DescriptorSetLayoutCreateInfo::default()
        }).unwrap();
        let pipeline_layout = PipelineLayout::new(Arc::clone(&selected_device), PipelineLayoutCreateInfo {
            flags: PipelineLayoutCreateFlags::default(),
            set_layouts: vec![
                Arc::clone(&descriptor_set_layout),
                Arc::clone(&texture_descriptor_set_layout),
            ],
            push_constant_ranges: vec![PushConstantRange {
                stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT | ShaderStages::GEOMETRY,
                offset: 0,
                size: 64 + 16,
            }],
            ..PipelineLayoutCreateInfo::default()
        }).unwrap();

        let descriptor_pool = DescriptorPool::new(Arc::clone(&selected_device), DescriptorPoolCreateInfo {
            max_sets: 11, // 1 universal descriptor set, 10 textures allowed.
            pool_sizes: [
                (DescriptorType::UniformBuffer, 3),
                (DescriptorType::SampledImage, 10),
                (DescriptorType::Sampler, 10),
            ].into_iter().collect(),
            flags: DescriptorPoolCreateFlags::empty(),
            ..Default::default()
        }).unwrap();
        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            Arc::clone(&selected_device),
            StandardDescriptorSetAllocatorCreateInfo {
                set_count: descriptor_set_layout.bindings().len() + texture_descriptor_set_layout.bindings().len() * 10,
                update_after_bind: false,
                ..Default::default()
            }
        );

        let descriptor_set = PersistentDescriptorSet::new(
            &descriptor_set_allocator,
            Arc::clone(&descriptor_set_layout),
            [
                WriteDescriptorSet::buffer(
                    0,
                    uniform_counts_buffer.clone().slice(0..16)
                ),
                WriteDescriptorSet::buffer_array(
                    1,
                    0,
                    (0..1024).map(|idx| {
                        let start = idx * 64;
                        let end = start + 64;
                        uniform_per_mesh_buffer.clone().slice(start..end)
                    })
                ),
                WriteDescriptorSet::buffer_array(
                    2,
                    0,
                    (0..1024).map(|idx| {
                        let start = idx * 32;
                        let end = start + 32;
                        uniform_light_buffer.clone().slice(start..end)
                    })
                ),
                WriteDescriptorSet::buffer_array(
                    3,
                    0,
                    (0..1024).map(|idx| {
                        let start = idx * 16;
                        let end = start + 16;
                        uniform_material_buffer.clone().slice(start..end)
                    })
                ),
            ],
            [],
        ).unwrap();
        let empty_texture_image = Image::new(
            Arc::clone(&device_memory_alloc) as _,
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::R32G32B32_SFLOAT,
                extent: [1, 1, 1],
                usage: ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo {
                ..Default::default()
            },
        ).unwrap();
        let empty_texture_image_view = ImageView::new_default(empty_texture_image).unwrap();
        let empty_texture_descriptor_set = PersistentDescriptorSet::new(
            &descriptor_set_allocator,
            Arc::clone(&texture_descriptor_set_layout),
            [
                WriteDescriptorSet::image_view(0, empty_texture_image_view),
                WriteDescriptorSet::sampler(1, Sampler::new(Arc::clone(&selected_device), SamplerCreateInfo::simple_repeat_linear_no_mipmap()).unwrap()),
            ],
            [],
        ).unwrap();
        let texture_sampler = Sampler::new(
            Arc::clone(&selected_device),
            SamplerCreateInfo {
                ..SamplerCreateInfo::simple_repeat_linear_no_mipmap()
            },
        ).unwrap();

        Ok(Self {
            vulkan,

            vertex_shader,
            geometry_shader,
            fragment_shader,

            ordered_physical_devices,
            selected_physical_device_idx,
            selected_device,
            selected_device_queues,

            device_memory_alloc,

            vertex_buffer,
            face_buffer,
            uniform_counts_buffer,
            uniform_per_mesh_buffer,
            uniform_light_buffer,
            uniform_material_buffer,
            staging_texture_buffer,
            constants_buffer,

            command_buffer_alloc,

            pipeline_layout,

            descriptor_set_layout,
            texture_descriptor_set_layout,
            descriptor_pool,
            descriptor_set_allocator,
            descriptor_set,
            empty_texture_descriptor_set,
            texture_sampler,

            // 1 since that's the typical usecase. 0 would be unused.
            windowed_swapchain: HashMap::with_capacity(1),

            loaded_models: HashMap::new(),
            index_free_byte_start: 0,
            index_free_start: 0,
            vertex_free_byte_start: 0,
            vertex_free_start: 0,
        })
    }

    fn copy_sized_slice_to_buffer<U: ?Sized, T: Sized + Copy + std::fmt::Debug + bytemuck::Pod>(buffer: &Subbuffer<U>, to_copy: &[T]) -> Result<(), HostAccessError> {
        let mapped_buffer = unsafe {
            buffer.mapped_slice()?.as_mut()
        };

        let cast_slice = bytemuck::cast_slice(to_copy);
        let num_bytes_to_copy = cast_slice.len();
        mapped_buffer[..num_bytes_to_copy].copy_from_slice(cast_slice);

        Ok(())
    }

    pub fn invalidate_model_memory(&mut self) {
        self.loaded_models.clear();
    }

    pub fn render_to<'a>(&mut self, window: Arc<Window>, task: RenderTask<'a>) -> Result<(), Validated<VulkanError>> {
        struct RenderTimer {
            start: Instant,
            inflight_load: Option<Instant>,
            loads: Vec<Duration>,
            inflight_command_record: Option<Instant>,
            command_record_duration: Option<Duration>,
            inflight_draw: Option<Instant>,
            draw_duration: Option<Duration>,
        }

        impl RenderTimer {
            fn begin() -> Self {
                Self {
                    start: Instant::now(),
                    inflight_load: None,
                    loads: Vec::with_capacity(20),
                    inflight_command_record: None,
                    command_record_duration: None,
                    inflight_draw: None,
                    draw_duration: None,
                }
            }

            fn record_load_start(&mut self) {
                self.inflight_load = Some(Instant::now());
            }

            fn record_load_end(&mut self) {
                let Some(start) = self.inflight_load else {
                    return;
                };
                self.loads.push(Instant::now() - start);
            }

            fn record_command_record_start(&mut self) {
                self.inflight_command_record = Some(Instant::now());
            }

            fn record_command_record_end(&mut self) {
                let Some(start) = self.inflight_command_record else {
                    return;
                };
                self.command_record_duration = Some(Instant::now() - start);
            }

            fn record_draw_start(&mut self) {
                self.inflight_draw = Some(Instant::now());
            }

            fn record_draw_end(&mut self) {
                let Some(start) = self.inflight_draw else {
                    return;
                };
                self.draw_duration = Some(Instant::now() - start);
            }

            fn record_end(self) {
                let over = Instant::now() - self.start;
                let a = 1000. / over.as_millis() as f64;
                log::info!("RENDER-PASS-TIMING fps={a} loads={:?} submit={:?} draw={:?} total={:?}", self.loads, self.command_record_duration, self.draw_duration, over);
            }
        }

        let mut perf = RenderTimer::begin();


        let mut e = self.windowed_swapchain.entry(window.id());
        let window_swapchain = match e {
            Entry::Vacant(v) => {
                v.insert(RendererWindowSwapchain::generate_swapchain(&self.vulkan, &window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device, &self.device_memory_alloc).unwrap())
            },
            Entry::Occupied(ref mut o) => {
                let swapchain_information = o.get_mut();
                if swapchain_information.is_stale_for_window(&window) {
                    swapchain_information.regenerated_swapchain(&window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device, &self.device_memory_alloc).unwrap();
                }
                swapchain_information
            },
        };

        log::info!("RENDER-PASS-INIT");

        {
            for draw in task.draws.iter() {
                let mesh_id = draw.mesh.mesh_id;
                if self.loaded_models.contains_key(&mesh_id) {
                    log::info!("RENDER-COPY mesh={mesh_id} already_loaded=true");
                    continue;
                }

                perf.record_load_start();

                Self::copy_sized_slice_to_buffer(&self.vertex_buffer.clone().slice(self.vertex_free_byte_start..), draw.mesh.vec_vv.as_slice()).unwrap();
                Self::copy_sized_slice_to_buffer(&self.face_buffer.clone().slice(self.index_free_byte_start..), draw.mesh.ff.as_slice()).unwrap();
                // We'll assume the image's format is RGB.
                let (texture_image, texture_descriptor_set) = if let Some(ref file_path) = draw.mesh.tex_file {
                    log::info!("RENDER-TEXTURE mesh={mesh_id} texture={file_path:?}");
                    assert!(file_path.ends_with(".png"), "only pngs are handled, and only ARGB pngs");
                    let f = std::fs::File::open(file_path).expect("provided file exists");
                    let texture_file = img::load(std::io::BufReader::new(f), ImageFormat::Png).unwrap().into_rgb32f();
                    let (texture_width, texture_height) = (texture_file.width(), texture_file.height());
                    Self::copy_sized_slice_to_buffer(&self.staging_texture_buffer.clone(), texture_file.into_raw().as_slice()).unwrap();
                    let texture_image = Image::new(
                        Arc::clone(&self.device_memory_alloc) as _,
                        ImageCreateInfo {
                            format: Format::R32G32B32_SFLOAT,
                            view_formats: vec![Format::R32G32B32_SFLOAT],
                            extent: [texture_width, texture_height, 1],
                            usage: ImageUsage::INPUT_ATTACHMENT | ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                            tiling: ImageTiling::Optimal,
                            initial_layout: ImageLayout::Undefined,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        },
                    ).unwrap();
                    let texture_image_view = ImageView::new_default(Arc::clone(&texture_image)).unwrap();

                    // TODO Batch these somehow.
                    let mut image_copy_builder = AutoCommandBufferBuilder::primary(&self.command_buffer_alloc, self.selected_device_queues[0].queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
                    image_copy_builder
                        .copy_buffer_to_image(CopyBufferToImageInfo {
                            ..CopyBufferToImageInfo::buffer_image(self.staging_texture_buffer.clone(), Arc::clone(&texture_image))
                        })
                        .unwrap();
                    let image_copy_buffer = image_copy_builder.build().unwrap();
                    vulkano::sync::now(Arc::clone(&self.selected_device))
                        .then_execute(Arc::clone(&self.selected_device_queues[0]), image_copy_buffer)
                        .unwrap()
                        .flush()
                        .unwrap();

                    let texture_descriptor_set = PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        Arc::clone(&self.texture_descriptor_set_layout),
                        [
                            WriteDescriptorSet::image_view(0, texture_image_view),
                            WriteDescriptorSet::sampler(
                                1,
                                Arc::clone(&self.texture_sampler),
                            ),
                        ],
                        [],
                    ).unwrap();

                    (Some(texture_image), Some(texture_descriptor_set))
                } else {
                    (None, None)
                };

                self.loaded_models.insert(draw.mesh.mesh_id,
                    LoadedModelHandles {
                        texture_image,
                        texture_descriptor_set,
                        vertex_offset: self.vertex_free_start,
                        index_offset: self.index_free_start,
                    }
                );

                self.vertex_free_start += draw.mesh.vec_vv.len() as i32;
                self.index_free_start += draw.mesh.ff.len() as u32;
                let vblen = bytemuck::cast_slice::<_, u8>(draw.mesh.vec_vv.as_slice()).len() as u64;
                let fblen = bytemuck::cast_slice::<_, u8>(draw.mesh.ff.as_slice()).len() as u64;
                self.vertex_free_byte_start += vblen;
                self.index_free_byte_start += fblen;

                log::info!("RENDER-COPY mesh={mesh_id} vertex_start={} vertex_len={vblen} face_start={} face_len={fblen}", self.vertex_free_byte_start, self.index_free_byte_start);

                perf.record_load_end();
            }

            perf.record_load_start();
            Self::copy_sized_slice_to_buffer(&self.uniform_per_mesh_buffer, task.instancing_information_bytes().as_slice()).unwrap();
            Self::copy_sized_slice_to_buffer(&self.uniform_light_buffer, task.lights.to_bytes().as_slice()).unwrap();
            Self::copy_sized_slice_to_buffer(&self.uniform_counts_buffer, &[0u32, task.lights.0.len() as u32, 0u32, 0u32]).unwrap();
            perf.record_load_end();
        }

        let (active_framebuffer, afidx, framebuffer_future) = {
            let (mut preferred, mut suboptimal, mut acquire_next_image) = swapchain::acquire_next_image(Arc::clone(&window_swapchain.swapchain), None).unwrap();
            const MAX_RECREATION_OCCURRENCES: usize = 3;
            let mut times_recreated = 0;
            while suboptimal && times_recreated < MAX_RECREATION_OCCURRENCES {
                times_recreated += 1;
                window_swapchain.regenerated_swapchain(&window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device, &self.device_memory_alloc).unwrap();
                let n = swapchain::acquire_next_image(Arc::clone(&window_swapchain.swapchain), None).unwrap();
                preferred = n.0;
                suboptimal = n.1;
                acquire_next_image = n.2;
            }
            (&window_swapchain.images[preferred as usize], preferred, acquire_next_image)
        };

        let subpass = Subpass::from(Arc::clone(&window_swapchain.render_pass), 0).unwrap();
        let pipeline = GraphicsPipeline::new(
            Arc::clone(&self.selected_device),
            None,
            GraphicsPipelineCreateInfo {
                stages: [
                    PipelineShaderStageCreateInfo::new(self.vertex_shader.entry_point("main").unwrap()),
                    PipelineShaderStageCreateInfo::new(self.fragment_shader.entry_point("main").unwrap()),
                    PipelineShaderStageCreateInfo::new(self.geometry_shader.entry_point("main").unwrap()),
                ].into_iter().collect(),
                rasterization_state: Some(RasterizationState {
                    cull_mode: CullMode::Back,
                    ..RasterizationState::default()
                }),
                input_assembly_state: Some(InputAssemblyState {
                    topology: PrimitiveTopology::TriangleList,
                    ..Default::default()
                }),
                vertex_input_state: Some(
                    VertexInputState::new()
                        .binding(
                            0,
                            VertexInputBindingDescription {
                                stride: 12 + 12 + 8,
                                input_rate: VertexInputRate::Vertex
                            },
                        )
                        .attribute(
                            0,
                            VertexInputAttributeDescription {
                                binding: 0,
                                format: Format::R32G32B32_SFLOAT,
                                offset: 0,
                            },
                        )
                        .binding(
                            1,
                            VertexInputBindingDescription {
                                stride: 12 + 12 + 8,
                                input_rate: VertexInputRate::Vertex
                            },
                        )
                        .attribute(
                            1,
                            VertexInputAttributeDescription {
                                binding: 0,
                                format: Format::R32G32B32_SFLOAT,
                                offset: 12,
                            },
                        )
                        .binding(
                            2,
                            VertexInputBindingDescription {
                                stride: 12 + 12 + 8,
                                input_rate: VertexInputRate::Vertex,
                            },
                        )
                        .attribute(
                            2,
                            VertexInputAttributeDescription {
                                binding: 0,
                                format: Format::R32G32_SFLOAT,
                                offset: 12 + 12,
                            },
                        )
                ),
                viewport_state: Some(ViewportState {
                    viewports: [Viewport {
                        offset: [0.0; 2],
                        extent: [active_framebuffer.image.extent()[0] as f32, active_framebuffer.image.extent()[1] as f32],
                        depth_range: 0.0..=1.0
                    }].into_iter().collect(),
                    scissors: [Scissor {
                        offset: [0; 2],
                        extent: [active_framebuffer.image.extent()[0], active_framebuffer.image.extent()[1]],
                    }].into_iter().collect(),
                    ..ViewportState::default()
                }),
                multisample_state: Some(MultisampleState {
                    rasterization_samples: SampleCount::Sample1,
                    ..Default::default()
                }),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState::default(),
                )),
                depth_stencil_state: Some(DepthStencilState {
                    depth: Some(DepthState::simple()),
                    ..Default::default()
                }),
                subpass: Some(PipelineSubpassType::BeginRenderPass(subpass)),
                ..GraphicsPipelineCreateInfo::layout(Arc::clone(&self.pipeline_layout))
            }
        ).unwrap();
        log::info!("RENDER-PASS-PIPELINE descriptor_set={}", pipeline.num_used_descriptor_sets());

        perf.record_command_record_start();

        let base_queue = &self.selected_device_queues[0];
        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_alloc,
            self.selected_device_queues[0].queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-RENDER-TO-FAILED source=clear_pipeline error=command_buffer_alloc {e}"))?;
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![
                        Some(task.clear_color.clone().into()),
                        Some(ClearValue::Depth(1f32))
                    ],
                    ..RenderPassBeginInfo::framebuffer(Arc::clone(&window_swapchain.images[0].framebuffer))
                },
                SubpassBeginInfo {
                    contents: SubpassContents::Inline,
                    ..Default::default()
                }
            )
            .unwrap()
            .clear_attachments(
                [
                    ClearAttachment::Color {
                        color_attachment: 0,
                        clear_value: task.clear_color,
                    }
                ].into_iter().collect(),
                [
                    ClearRect {
                        offset: [0, 0],
                        extent: [active_framebuffer.image.extent()[0], active_framebuffer.image.extent()[1]],
                        array_layers: 0..1,
                    }
                ].into_iter().collect()
            )
            .unwrap()
            .bind_pipeline_graphics(pipeline)
            .unwrap()
            .bind_vertex_buffers(0, self.vertex_buffer.clone())
            .unwrap()
            .bind_vertex_buffers(1, self.vertex_buffer.clone())
            .unwrap()
            .bind_vertex_buffers(2, self.vertex_buffer.clone())
            .unwrap()
            .push_constants(Arc::clone(&self.pipeline_layout), 0, task.cam.get_vp_mat())
            .unwrap()
            .push_constants(Arc::clone(&self.pipeline_layout), 64, [if task.draw_wireframe { 1u32 } else { 0u32 }, 0u32, 0u32, 0u32])
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                Arc::clone(&self.pipeline_layout),
                0,
                (Arc::clone(&self.descriptor_set), Arc::clone(&self.empty_texture_descriptor_set))
            )
            .unwrap()
            .bind_index_buffer(IndexBuffer::U32(self.face_buffer.clone().reinterpret()))
            .unwrap();
        let mut current_instance_buffer_idx = 0;
        for draw in task.draws.iter() {
            let handle = &self.loaded_models[&draw.mesh.mesh_id];
            let vert_count = draw.mesh.vec_vv.len() as i32;
            let index_count = draw.mesh.ff.len() as u32;
            let instance_count = draw.instancing_information.len() as u32;
            log::info!(
                "RENDER-PASS-DRAW vertex_start={} vertex_count={vert_count} index_start={} index_count={index_count} instance_start={current_instance_buffer_idx} instance_count={instance_count}",
                handle.vertex_offset,
                handle.index_offset,
            );
            let texture_descriptor_set = if let Some(ref descriptor_set) = handle.texture_descriptor_set {
                descriptor_set
            } else {
                &self.empty_texture_descriptor_set
            };
            builder
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    Arc::clone(&self.pipeline_layout),
                    1,
                    Arc::clone(&texture_descriptor_set),
                )
                .unwrap();
            builder
                .draw_indexed(
                    index_count,
                    instance_count,
                    handle.index_offset,
                    handle.vertex_offset,
                    current_instance_buffer_idx,
                )
                .unwrap();
            current_instance_buffer_idx += instance_count;
            log::info!("RENDER-PASS-DRAW-COMPLETE");
        }
        builder
            .end_render_pass(SubpassEndInfo { ..Default::default() })
            .unwrap();
        let clear_buffer = builder.build().unwrap();

        perf.record_command_record_end();

        perf.record_draw_start();

        vulkano::sync::now(Arc::clone(&self.selected_device))
            .join(framebuffer_future)
            .then_execute(Arc::clone(&base_queue), clear_buffer)
            .unwrap()
            .then_swapchain_present(Arc::clone(base_queue), SwapchainPresentInfo::swapchain_image_index(Arc::clone(&window_swapchain.swapchain), afidx))
            .flush()
            .unwrap();

        perf.record_draw_end();

        perf.record_end();
        log::info!("RENDER-PASS-COMPLETE");

        Ok(())
    }
}

pub struct DepthImage {
    pub view: Arc<ImageView>,
    pub image: Arc<Image>,
}

pub struct FramebufferedImage {
    pub framebuffer: Arc<Framebuffer>,
    pub view: Arc<ImageView>,
    pub image: Arc<Image>,
}

pub struct RendererWindowSwapchain {
     cached_dimensions: PhysicalSize<u32>,
     surface: Arc<Surface>,
     composite_alpha: CompositeAlpha,
     swapchain: Arc<Swapchain>,
     render_pass: Arc<RenderPass>,
     depth_image: DepthImage,
     images: Vec<FramebufferedImage>,
}

impl RendererWindowSwapchain {
    fn is_stale_for_window(&self, window: &Arc<Window>) -> bool {
        let dimensions = window.inner_size();
        self.cached_dimensions == dimensions
    }

    fn generate_swapchain(vulkan: &Arc<Instance>, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>, mem_alloc: &Arc<StandardMemoryAllocator>) -> Result<Self, Validated<VulkanError>> {
        let surface = Surface::from_window(
            Arc::clone(&vulkan),
            Arc::clone(window)
        ).unwrap();

        let (dimensions, composite_alpha, render_pass, swapchain, depth_image, images) = Self::generate_swapchain_from_surface(&surface, window, pd, device, mem_alloc)?;

        Ok(RendererWindowSwapchain { cached_dimensions: dimensions, surface, composite_alpha, swapchain, render_pass, depth_image, images })
    }

    fn regenerated_swapchain(&mut self, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>, mem_alloc: &Arc<StandardMemoryAllocator>) -> Result<(), Validated<VulkanError>> {
        let (dimensions, composite_alpha, render_pass, swapchain, depth_image, images) = Self::generate_swapchain_from_surface(&self.surface, window, pd, device, mem_alloc)?;

        self.cached_dimensions = dimensions;
        self.composite_alpha = composite_alpha;
        self.swapchain = swapchain;
        self.render_pass = render_pass;
        self.depth_image = depth_image;
        self.images = images;

        Ok(())
    }

    fn generate_swapchain_from_surface(surface: &Arc<Surface>, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>, mem_alloc: &Arc<StandardMemoryAllocator>) -> Result<(
        PhysicalSize<u32>,
        CompositeAlpha,
        Arc<RenderPass>,
        Arc<Swapchain>,
        DepthImage,
        Vec<FramebufferedImage>,
    ), Validated<VulkanError>> {
        let capabilities = pd.surface_capabilities(&surface, Default::default())
            .tap_err(|e| log::error!("TOTALITY-RENDERER-RENDER-TO-FAILED source=surface_capability {e}"))?;
        let dimensions = window.inner_size();
        // Assume one is available.
        let composite_alpha = capabilities.supported_composite_alpha.into_iter().next().unwrap();
        let format = pd
            .surface_formats(&surface, Default::default())
            .expect("surface lookup done")[0].0;

        let (swapchain, raw_images) = Swapchain::new(
            Arc::clone(device),
            Arc::clone(surface),
            SwapchainCreateInfo {
                composite_alpha,
                image_format: format,
                image_extent: dimensions.into(),
                image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                min_image_count: capabilities.min_image_count + 1,
                ..Default::default()
            },
        )?;

        let depth_image = Image::new(
            Arc::clone(mem_alloc) as _,
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::D16_UNORM,
                extent: raw_images[0].extent(),
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo {
                ..Default::default()
            },
        ).unwrap();
        let depth_attachment = ImageView::new_default(Arc::clone(&depth_image)).unwrap();

        let render_pass = vulkano::single_pass_renderpass!(
            Arc::clone(device),
            attachments: {
                color: {
                    format: format,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth_stencil: {
                    format: Format::D16_UNORM,
                    samples: 1,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {depth_stencil},
            },
        ).tap_err(|e| log::error!("TOTALITY-RENDERER-RENDER-TO-FAILED source=render_pass {e}"))?;


        let images = raw_images.into_iter().map(|image| {
            let view = ImageView::new_default(Arc::clone(&image))?;
            let framebuffer = Framebuffer::new(
                Arc::clone(&render_pass),
                vulkano::render_pass::FramebufferCreateInfo {
                    attachments: vec![Arc::clone(&view), Arc::clone(&depth_attachment)],
                    ..Default::default()
                },
            )?;
            Ok(FramebufferedImage {
                image,
                view,
                framebuffer,
            })
        }).collect::<Result<Vec<_>, Validated<VulkanError>>>()?;

        Ok((dimensions, composite_alpha, render_pass, swapchain, DepthImage {
            image: depth_image,
            view: depth_attachment,
        }, images))
    }
}

