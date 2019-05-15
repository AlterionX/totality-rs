use super::hal::{
    adapter::MemoryTypeId,
    buffer::Usage,
    mapping::Writer,
    memory::{Properties, Requirements},
    pso::{EntryPoint, Specialization},
    Adapter, Backend, Device, PhysicalDevice,
};
use std::{marker::PhantomData, mem::ManuallyDrop, sync::Arc};

#[allow(dead_code)]
use log::{debug, error, info, trace, warn};

pub struct AllocatedBuffer<B: Backend> {
    mem: ManuallyDrop<<B>::Memory>,
    reqs: Requirements,
    buf: ManuallyDrop<<B>::Buffer>,
    pub usage: Usage,
    dev: PhantomData<<B>::Device>,
    name: String,
    dropped: bool,
}
impl<B: Backend> AllocatedBuffer<B> {
    pub fn new(
        adapter: &Adapter<B>,
        dev: &<B>::Device,
        name: Option<String>,
        sz: u64,
        usage: Usage,
    ) -> Result<Self, &'static str> {
        let name = name.unwrap_or("<unknown>".to_string());
        let mut buffer =
            unsafe { dev.create_buffer(sz, usage) }.map_err(|_| "Couldn't create a buffer!")?;
        let requirements = unsafe { dev.get_buffer_requirements(&buffer) };
        let memory_type_id = adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .ok_or("Couldn't find a memory type to support the buffer!")?;
        let memory = unsafe { dev.allocate_memory(memory_type_id, requirements.size) }
            .map_err(|_| "Couldn't allocate buffer memory")?;
        unsafe { dev.bind_buffer_memory(&memory, 0, &mut buffer) }
            .map_err(|_| "Couldn't bind the buffer memory!")?;
        Result::Ok(AllocatedBuffer {
            buf: ManuallyDrop::new(buffer),
            reqs: requirements,
            mem: ManuallyDrop::new(memory),
            usage: usage,
            dev: PhantomData,
            name: name,
            dropped: false,
        })
    }
    pub fn load_data<T: Copy, F: FnOnce(&mut Writer<B, T>)>(
        &self,
        device: &<B>::Device,
        loader: F,
    ) -> Result<(), &'static str> {
        unsafe {
            let mut data_target = device
                .acquire_mapping_writer(&self.mem, 0..self.reqs.size)
                .map_err(|_| "Failed to acquire a memory writer!")?;
            loader(&mut data_target);
            device
                .release_mapping_writer(data_target)
                .map_err(|_| "Couldn't release the mapping writer!")?;
        }
        Ok(())
    }
    pub fn load_data_from_slice(
        &self,
        device: &<B>::Device,
        sliced: &[u32],
        dst_offset: usize,
    ) -> Result<(), &'static str> {
        self.load_data(device, |target| {
            target[dst_offset..(dst_offset + sliced.len())].copy_from_slice(&sliced);
        });
        Ok(())
    }
    pub fn free(mut self, dev: &<B>::Device) {
        if !self.dropped {
            unsafe {
                use std::ptr::read;
                dev.destroy_buffer(ManuallyDrop::into_inner(read(&mut self.buf)));
                dev.free_memory(ManuallyDrop::into_inner(read(&mut self.mem)));
                ManuallyDrop::drop(&mut self.mem);
                ManuallyDrop::drop(&mut self.buf);
                self.dropped = true;
            }
        }
    }
    pub fn buffer_ref(&self) -> &<B>::Buffer {
        &self.buf
    }
}
impl<B: Backend> Drop for AllocatedBuffer<B> {
    fn drop(&mut self) {
        if !self.dropped {
            panic!("Allocated buffers must be manually dropped!");
        }
    }
}

pub struct LoadedBuffer<T, B: Backend> {
    source: Option<Arc<T>>,
    pub buffer: AllocatedBuffer<B>,
}
impl<T, B: Backend> LoadedBuffer<T, B> {
    pub fn new(
        adapter: &Adapter<B>,
        dev: &mut <B>::Device,
        name: Option<String>,
        sz: u64,
        usage: Usage,
        data: &[u32],
        source: Arc<T>,
    ) -> Result<Self, &'static str> {
        trace!("Requested creation of LoadedBuffer with capacity {:?}.", sz);
        let ab = AllocatedBuffer::new(adapter, dev, name, sz, usage)?;
        ab.load_data_from_slice(dev, data, 0)?;
        Ok(LoadedBuffer {
            source: Some(source),
            buffer: ab,
        })
    }
    pub fn free(mut self, dev: &<B>::Device) {
        self.source.take();
        self.buffer.free(dev)
    }
    pub fn buffer_ref(&self) -> &<B>::Buffer {
        self.buffer.buffer_ref()
    }
    pub fn matches_source(&self, check: &Arc<T>) -> bool {
        if let Some(ref a) = self.source {
            Arc::ptr_eq(a, check)
        } else {
            false
        }
    }
}
