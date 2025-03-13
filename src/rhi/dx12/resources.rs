use std::ffi::CString;

use oxidx::dx::{self, IDevice, IDeviceChildExt, IResource};
use parking_lot::{Mutex, MutexGuard};

use crate::rhi::resources::{
    BufferDesc, BufferUsages, MemoryLocation, RenderResourceDevice, SamplerDesc, TextureDesc,
    TextureViewDesc,
};

use super::device::DxDevice;

impl RenderResourceDevice for DxDevice {
    type Buffer = DxBuffer;
    type Texture = ();
    type Sampler = ();

    fn create_buffer(&self, desc: BufferDesc) -> Self::Buffer {
        let heap_props = match desc.memory_location {
            MemoryLocation::CpuToGpu => dx::HeapProperties::upload(),
            MemoryLocation::GpuToGpu => dx::HeapProperties::default(),
            MemoryLocation::GpuToCpu => dx::HeapProperties::readback(),
        };

        let raw_desc = dx::ResourceDesc::buffer(desc.size).with_layout(dx::TextureLayout::RowMajor);

        let initial_state = if desc.usage.contains(BufferUsages::Uniform)
            | desc.usage.contains(BufferUsages::Copy)
        {
            dx::ResourceStates::GenericRead
        } else if desc.usage.contains(BufferUsages::QueryResolve) {
            dx::ResourceStates::CopyDest
        } else {
            dx::ResourceStates::Common
        };

        let raw = self
            .gpu
            .create_committed_resource(
                &heap_props,
                dx::HeapFlags::empty(),
                &raw_desc,
                initial_state,
                None,
            )
            .expect("Failed to create buffer");

        if let Some(name) = &desc.name {
            let debug_name = CString::new(name.as_bytes()).expect("failed to create resource name");
            raw.set_debug_object_name(&debug_name)
                .expect("failed to set debug object name");
        }

        DxBuffer {
            raw,
            desc,
            state: Mutex::new(initial_state),
            map_guard: Mutex::new(()),
        }
    }

    fn destroy_buffer(&self, _buffer: Self::Buffer) {}

    fn create_texture(&self, desc: TextureDesc) -> Self::Texture {
        todo!()
    }

    fn destroy_texture(&self, texture: Self::Texture) {
        todo!()
    }

    fn create_texture_view(&self, texture: &Self::Texture, desc: TextureViewDesc) -> Self::Texture {
        todo!()
    }

    fn open_texture(&self, texture: &Self::Texture, other_gpu: &Self) -> Self::Texture {
        todo!()
    }

    fn create_sampler(&self, desc: SamplerDesc) -> Self::Sampler {
        todo!()
    }

    fn destroy_sampler(&self, _sampler: Self::Sampler) {}
}

#[derive(Debug)]
pub struct DxBuffer {
    pub(super) raw: dx::Resource,
    pub(super) desc: BufferDesc,
    pub(super) state: Mutex<dx::ResourceStates>,

    map_guard: Mutex<()>,
}

impl DxBuffer {
    pub fn map<T>(&self) -> BufferMap<'_, T> {
        let size = self.desc.size / size_of::<T>();

        let pointer = self.raw.map::<T>(0, None).expect("Failed to map buffer");

        unsafe {
            let pointer = std::slice::from_raw_parts_mut(pointer.as_ptr(), size);
            let guard = self.map_guard.lock();

            BufferMap {
                _guard: guard,
                pointer,
            }
        }
    }
}

#[derive(Debug)]
pub struct BufferMap<'a, T> {
    _guard: MutexGuard<'a, ()>,
    pub pointer: &'a mut [T],
}
