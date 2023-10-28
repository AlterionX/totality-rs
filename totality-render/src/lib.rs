pub mod task;
mod shaders;

use std::{sync::Arc, collections::{HashMap, hash_map::Entry}};

use na::Matrix4;
use tap::{TapFallible, TapOptional};
use task::RenderTask;
use thiserror::Error;

use vulkano::{
    VulkanLibrary,
    VulkanError,
    instance::{
        Instance,
        InstanceCreateInfo, InstanceExtensions,
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
        DeviceExtensions,
    },
    memory::allocator::{
        StandardMemoryAllocator,
        AllocationCreateInfo,
        MemoryTypeFilter,
        MemoryAllocator,
    },
    buffer::{
        Buffer,
        BufferCreateInfo,
        BufferUsage,
        Subbuffer,
        AllocateBufferError,
    },
    command_buffer::{
        allocator::{
            StandardCommandBufferAllocator,
            StandardCommandBufferAllocatorCreateInfo,
        },
        AutoCommandBufferBuilder,
        CommandBufferUsage,
        RenderPassBeginInfo,
        SubpassBeginInfo,
        SubpassContents,
        SubpassEndInfo,
        ClearAttachment,
        ClearRect,
    },
    image::{
        ImageUsage,
        Image,
        view::ImageView,
        SampleCount,
    },
    format::{
        Format,
        ClearColorValue,
    },
    render_pass::{
        RenderPass,
        Framebuffer,
        Subpass,
    },
    shader::{ShaderModule, ShaderStages},
    sync::{
        HostAccessError,
        GpuFuture,
    },
    pipeline::{
        GraphicsPipeline,
        graphics::{
            GraphicsPipelineCreateInfo,
            rasterization::RasterizationState,
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
        },
        PipelineLayout,
        layout::{
            PipelineLayoutCreateInfo,
            PipelineLayoutCreateFlags, PushConstantRange,
        },
        PipelineShaderStageCreateInfo, PipelineBindPoint,
    }, descriptor_set::{pool::{DescriptorPool, DescriptorPoolCreateInfo, DescriptorPoolCreateFlags, DescriptorSetAllocateInfo}, layout::{DescriptorType, DescriptorSetLayout, DescriptorSetLayoutCreateInfo, DescriptorSetLayoutCreateFlags, DescriptorSetLayoutBinding, DescriptorBindingFlags}, allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo, DescriptorSetAllocator, StandardDescriptorSetAlloc}, PersistentDescriptorSet, WriteDescriptorSet},
};
use winit::{window::{WindowId, Window}, dpi::PhysicalSize};

use model::geom::tri::TriMeshGeom;

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
}

pub struct Renderer {
    vulkan: Arc<Instance>,

    vertex_shader: Arc<ShaderModule>,
    fragment_shader: Arc<ShaderModule>,

    ordered_physical_devices: Vec<Arc<PhysicalDevice>>,
    selected_physical_device_idx: usize,
    selected_device: Arc<Device>,
    selected_device_queues: Vec<Arc<Queue>>,

    device_memory_alloc: Arc<StandardMemoryAllocator>,

    vertex_buffer: Subbuffer<[u8]>,
    face_buffer: Subbuffer<[u8]>,
    uniform_per_mesh_buffer: Subbuffer<[u8]>,
    uniform_light_buffer: Subbuffer<[u8]>,
    constants_buffer: Subbuffer<[u8]>,
    texture_buffer: Subbuffer<[u8]>,

    command_buffer_alloc: Arc<StandardCommandBufferAllocator>,

    pipeline_layout: Arc<PipelineLayout>,

    descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_pool: DescriptorPool,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    descriptor_set: Arc<PersistentDescriptorSet>,

    // TODO: better map for window ids?
    windowed_swapchain: HashMap<WindowId, RendererWindowSwapchain>,
}

impl Renderer {
    pub fn init(application_name: Option<String>, application_version: Option<Version>, windowing: &Window, pref: &RendererPreferences) -> Result<Self, RendererInitializationError> {
        let vulkan_library = VulkanLibrary::new()
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED primary_source=vulkan_lib error=missing_error {e}"))?;
        let required_extensions = Surface::required_extensions(windowing);
        let vulkan = Instance::new(
            vulkan_library,
            InstanceCreateInfo {
                enabled_extensions: required_extensions,
                engine_name: Some("totality".to_owned()),
                engine_version: Version::default(),
                application_name,
                application_version: application_version.unwrap_or_default(),
                enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_owned(), "VK_LAYER_LUNARG_api_dump".to_owned()],
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
        fn upcast(a: Arc<StandardMemoryAllocator>) -> Arc<dyn MemoryAllocator> {
            a
        }
        let dyn_device_memory_alloc = upcast(Arc::clone(&device_memory_alloc));

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
            16 * 1024,
        )
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=matrix_buffer error=failed_creation {e}"))
            .map_err(RendererInitializationError::BufferCreationFailed)?;
        // And another for textures.
        let texture_buffer = Buffer::new_unsized(
            Arc::clone(&dyn_device_memory_alloc),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_TEXEL_BUFFER | BufferUsage::TRANSFER_DST,
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
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=texture_buffer error=failed_creation {e}"))
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
        let fragment_shader = shaders::basic_frag::load(Arc::clone(&selected_device))
            .tap_err(|e| log::error!("TOTALITY-RENDERER-INIT-FAILED source=shader shader=basic_vert {e}"))
            .map_err(|e| RendererInitializationError::ShaderLoadFail("basic_vert", e))?;

        let descriptor_set_layout = DescriptorSetLayout::new(Arc::clone(&selected_device), DescriptorSetLayoutCreateInfo {
            flags: DescriptorSetLayoutCreateFlags::empty(),
            bindings: [
                (0, DescriptorSetLayoutBinding {
                    descriptor_count: 1024,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
                (1, DescriptorSetLayoutBinding {
                    descriptor_count: 1024,
                    stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                }),
            ].into_iter().collect(),
            ..DescriptorSetLayoutCreateInfo::default()
        }).unwrap();
        let pipeline_layout = PipelineLayout::new(Arc::clone(&selected_device), PipelineLayoutCreateInfo {
            flags: PipelineLayoutCreateFlags::default(),
            set_layouts: vec![Arc::clone(&descriptor_set_layout)],
            push_constant_ranges: vec![PushConstantRange {
                stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                offset: 0,
                size: 64 + 16,
            }],
            ..PipelineLayoutCreateInfo::default()
        }).unwrap();

        let descriptor_pool = DescriptorPool::new(Arc::clone(&selected_device), DescriptorPoolCreateInfo {
            max_sets: 20,
            pool_sizes: [
                (DescriptorType::UniformBuffer, 2),
            ].into_iter().collect(),
            flags: DescriptorPoolCreateFlags::empty(),
            ..Default::default()
        }).unwrap();
        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            Arc::clone(&selected_device),
            StandardDescriptorSetAllocatorCreateInfo {
                set_count: descriptor_set_layout.bindings().len(),
                update_after_bind: false,
                ..Default::default()
            }
        );

        let descriptor_set = PersistentDescriptorSet::new(
            &descriptor_set_allocator,
            Arc::clone(&pipeline_layout.set_layouts().get(0).unwrap()),
            [
                WriteDescriptorSet::buffer_array(
                    0,
                    0,
                    (0..1024).map(|idx| {
                        let start = idx * 64;
                        let end = start + 64;
                        uniform_per_mesh_buffer.clone().slice(start..end)
                    })
                ),
                WriteDescriptorSet::buffer_array(
                    0,
                    0,
                    (0..1024).map(|idx| {
                        let start = idx * 16;
                        let end = start + 16;
                        uniform_light_buffer.clone().slice(start..end)
                    })
                ),
            ],
            [],
        ).unwrap();

        Ok(Self {
            vulkan,

            vertex_shader,
            fragment_shader,

            ordered_physical_devices,
            selected_physical_device_idx,
            selected_device,
            selected_device_queues,

            device_memory_alloc,

            vertex_buffer,
            face_buffer,
            uniform_per_mesh_buffer,
            uniform_light_buffer,
            texture_buffer,
            constants_buffer,

            command_buffer_alloc,

            pipeline_layout,

            descriptor_set_layout,
            descriptor_pool,
            descriptor_set_allocator,
            descriptor_set,

            // 1 since that's the typical usecase. 0 would be unused.
            windowed_swapchain: HashMap::with_capacity(1),
        })
    }

    fn copy_sized_slice_to_buffer<U: ?Sized, T: Sized + Copy + std::fmt::Debug + bytemuck::Pod>(buffer: &Subbuffer<U>, to_copy: &[T]) -> Result<(), HostAccessError> {
        let mapped_buffer = unsafe {
            buffer.mapped_slice()?.as_mut()
        };

        let num_bytes_to_copy = to_copy.len() * std::mem::size_of::<T>();
        mapped_buffer[..num_bytes_to_copy].copy_from_slice(bytemuck::cast_slice(to_copy));

        Ok(())
    }

    pub fn load_model(
        &self,
        mesh: TriMeshGeom,
    ) -> Result<(), HostAccessError> {
        // TODO This copying stuff can be better.

        Self::copy_sized_slice_to_buffer(&self.vertex_buffer, mesh.vec_vv.as_slice())?;
        Self::copy_sized_slice_to_buffer(&self.face_buffer, mesh.vec_ff.as_slice())?;

        Ok(())
    }

    pub fn render_to<'a>(&mut self, window: Arc<Window>, task: RenderTask<'a>) -> Result<(), Validated<VulkanError>> {
        let mut e = self.windowed_swapchain.entry(window.id());
        let window_swapchain = match e {
            Entry::Vacant(v) => {
                v.insert(RendererWindowSwapchain::generate_swapchain(&self.vulkan, &window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device).unwrap())
            },
            Entry::Occupied(ref mut o) => {
                let swapchain_information = o.get_mut();
                if swapchain_information.is_stale_for_window(&window) {
                    swapchain_information.regenerated_swapchain(&window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device).unwrap();
                }
                swapchain_information
            },
        };

        log::info!("RENDER-PASS-INIT");

        Self::copy_sized_slice_to_buffer(
            &self.uniform_per_mesh_buffer, task.instancing_information_bytes().as_slice()
        ).unwrap();

        let (active_framebuffer, afidx, framebuffer_future) = {
            let (mut preferred, mut suboptimal, mut acquire_next_image) = swapchain::acquire_next_image(Arc::clone(&window_swapchain.swapchain), None).unwrap();
            const MAX_RECREATION_OCCURRENCES: usize = 3;
            let mut times_recreated = 0;
            while suboptimal && times_recreated < MAX_RECREATION_OCCURRENCES {
                times_recreated += 1;
                window_swapchain.regenerated_swapchain(&window, &self.ordered_physical_devices[self.selected_physical_device_idx], &self.selected_device).unwrap();
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
                ].into_iter().collect(),
                rasterization_state: Some(RasterizationState::default()),
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
                subpass: Some(PipelineSubpassType::BeginRenderPass(subpass)),
                ..GraphicsPipelineCreateInfo::layout(Arc::clone(&self.pipeline_layout))
            }
        ).unwrap();

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
                    clear_values: vec![Some(task.clear_color.clone().into())],
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
            .push_constants(Arc::clone(&self.pipeline_layout), 64, 0u32)
            .unwrap()
            .push_constants(Arc::clone(&self.pipeline_layout), 68, 0u32)
            .unwrap()
            .push_constants(Arc::clone(&self.pipeline_layout), 72, 0u32)
            .unwrap()
            .push_constants(Arc::clone(&self.pipeline_layout), 76, 0u32)
            .unwrap()
            .bind_descriptor_sets(PipelineBindPoint::Graphics, Arc::clone(&self.pipeline_layout), 0, Arc::clone(&self.descriptor_set))
            .unwrap()
            .draw(3, 1, 0, 0)
            .unwrap()
            .end_render_pass(SubpassEndInfo { ..Default::default() })
            .unwrap();
        let clear_buffer = builder.build().unwrap();
        vulkano::sync::now(Arc::clone(&self.selected_device))
            .join(framebuffer_future)
            .then_execute(Arc::clone(&base_queue), clear_buffer)
            .unwrap()
            .then_swapchain_present(Arc::clone(base_queue), SwapchainPresentInfo::swapchain_image_index(Arc::clone(&window_swapchain.swapchain), afidx))
            .flush()
            .unwrap();

        log::info!("RENDER-PASS-COMPLETE");

        Ok(())
    }
}

pub struct FramebufferedImage {
    framebuffer: Arc<Framebuffer>,
    view: Arc<ImageView>,
    image: Arc<Image>,
}

pub struct RendererWindowSwapchain {
     cached_dimensions: PhysicalSize<u32>,
     surface: Arc<Surface>,
     composite_alpha: CompositeAlpha,
     swapchain: Arc<Swapchain>,
     render_pass: Arc<RenderPass>,
     images: Vec<FramebufferedImage>,
}

impl RendererWindowSwapchain {
    fn is_stale_for_window(&self, window: &Arc<Window>) -> bool {
        let dimensions = window.inner_size();
        self.cached_dimensions == dimensions
    }

    fn generate_swapchain(vulkan: &Arc<Instance>, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>) -> Result<Self, Validated<VulkanError>> {
        let surface = Surface::from_window(
            Arc::clone(&vulkan),
            Arc::clone(window)
        )?;

        let (dimensions, composite_alpha, render_pass, swapchain, images) = Self::generate_swapchain_from_surface(&surface, window, pd, device)?;

        Ok(RendererWindowSwapchain { cached_dimensions: dimensions, surface, composite_alpha, swapchain, render_pass, images })
    }

    fn regenerated_swapchain(&mut self, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>) -> Result<(), Validated<VulkanError>> {
        let (dimensions, composite_alpha, render_pass, swapchain, images) = Self::generate_swapchain_from_surface(&self.surface, window, pd, device)?;

        self.cached_dimensions = dimensions;
        self.composite_alpha = composite_alpha;
        self.swapchain = swapchain;
        self.render_pass = render_pass;
        self.images = images;

        Ok(())
    }

    fn generate_swapchain_from_surface(surface: &Arc<Surface>, window: &Arc<Window>, pd: &Arc<PhysicalDevice>, device: &Arc<Device>) -> Result<(
        PhysicalSize<u32>,
        CompositeAlpha,
        Arc<RenderPass>,
        Arc<Swapchain>,
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

        let render_pass = vulkano::single_pass_renderpass!(
            Arc::clone(device),
            attachments: {
                color: {
                    format: format,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {},
            },
        ).tap_err(|e| log::error!("TOTALITY-RENDERER-RENDER-TO-FAILED source=render_pass {e}"))?;


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

        let images = raw_images.into_iter().map(|image| {
            let view = ImageView::new_default(Arc::clone(&image))?;
            let framebuffer = Framebuffer::new(
                Arc::clone(&render_pass),
                vulkano::render_pass::FramebufferCreateInfo {
                    attachments: vec![Arc::clone(&view)],
                    ..Default::default()
                },
            )?;
            Ok(FramebufferedImage {
                image,
                view,
                framebuffer,
            })
        }).collect::<Result<Vec<_>, Validated<VulkanError>>>()?;

        Ok((dimensions, composite_alpha, render_pass, swapchain, images))
    }
}

