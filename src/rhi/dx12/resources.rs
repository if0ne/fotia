use std::ffi::CString;

use oxidx::dx::{self, IDevice, IDeviceChildExt, IResource};
use parking_lot::{Mutex, MutexGuard};

use crate::rhi::{
    dx12::conv::map_texture_desc,
    resources::{
        BufferDesc, BufferUsages, MemoryLocation, RenderResourceDevice, SamplerDesc, TextureDesc,
        TextureType, TextureUsages, TextureViewDesc, TextureViewType,
    },
};

use super::{
    conv::{map_format, map_texture_flags},
    device::{Descriptor, DxDevice},
};

impl RenderResourceDevice for DxDevice {
    type Buffer = DxBuffer;
    type Texture = DxTexture;
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
        if desc.usage.contains(TextureUsages::Shared) {
            let raw_desc = map_texture_desc(&desc, self.desc.is_cross_adapter_texture_supported);

            let raw_desc = if raw_desc
                .flags()
                .contains(dx::ResourceFlags::AllowCrossAdapter)
            {
                raw_desc.with_layout(dx::TextureLayout::RowMajor)
            } else {
                raw_desc
                    .with_flags(dx::ResourceFlags::AllowCrossAdapter)
                    .with_layout(dx::TextureLayout::RowMajor)
            };

            let size = self
                .gpu
                .get_copyable_footprints(&raw_desc, 0..1, 0, None, None, None);

            let heap = self
                .gpu
                .create_heap(
                    &dx::HeapDesc::new(size * 2, dx::HeapProperties::default())
                        .with_flags(dx::HeapFlags::SharedCrossAdapter | dx::HeapFlags::Shared),
                )
                .expect("Failed to create shared heap");

            self.create_shared_texture(desc, heap)
        } else {
            self.create_local_texture(desc)
        }
    }

    fn destroy_texture(&self, texture: Self::Texture) {
        if let Some(descriptor) = texture.descriptor {
            self.descriptors.free(descriptor);
        }
    }

    fn create_texture_view(&self, texture: &Self::Texture, desc: TextureViewDesc) -> Self::Texture {
        todo!()
    }

    fn open_texture(&self, texture: &Self::Texture, other_gpu: &Self) -> Self::Texture {
        let heap = match &texture.flavor {
            TextureFlavor::Local => panic!("Texture is local, can not open handle"),
            TextureFlavor::CrossAdapter { heap } => heap,
            TextureFlavor::Binded { heap, .. } => heap,
        };

        let handle = other_gpu
            .gpu
            .create_shared_handle(heap, None)
            .expect("Failed to open handle");
        let open_heap: dx::Heap = self
            .gpu
            .open_shared_handle(handle)
            .expect("Failed to open heap");
        handle.close().expect("Failed to close handle");

        self.create_shared_texture(
            texture
                .desc
                .clone()
                .with_name(std::borrow::Cow::Owned(format!(
                    "{} Opened",
                    texture
                        .desc
                        .name
                        .as_ref()
                        .unwrap_or(&std::borrow::Cow::Borrowed("Unnamed"))
                ))),
            open_heap,
        )
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

#[derive(Debug)]
pub struct DxTexture {
    pub(super) raw: dx::Resource,
    pub(super) state: Mutex<dx::ResourceStates>,
    pub(super) desc: TextureDesc,
    pub(super) flavor: TextureFlavor,

    pub(super) size: usize,

    pub(super) descriptor: Option<Descriptor>,
    pub(super) view: TextureViewDesc,
    pub(super) is_view: bool,
}

#[derive(Debug)]
pub enum TextureFlavor {
    Local,
    CrossAdapter {
        heap: dx::Heap,
    },
    Binded {
        heap: dx::Heap,
        cross: dx::Resource,
        cross_state: Mutex<dx::ResourceStates>,
    },
}

impl DxDevice {
    fn create_local_texture(&self, desc: TextureDesc) -> DxTexture {
        let raw_desc = map_texture_desc(&desc, self.desc.is_cross_adapter_texture_supported);

        let size = self.gpu.get_copyable_footprints(
            &raw_desc,
            0..(desc.subresource_count()),
            0,
            None,
            None,
            None,
        );

        let raw = self
            .gpu
            .create_committed_resource(
                &dx::HeapProperties::default(),
                dx::HeapFlags::empty(),
                &raw_desc,
                dx::ResourceStates::Common,
                None,
            )
            .expect("Failed to create buffer");

        let view = desc.to_default_view();

        let descriptor = if desc.usage.contains(TextureUsages::RenderTarget) {
            Some(self.descriptors.allocate(dx::DescriptorHeapType::Rtv, 1))
        } else if desc.usage.contains(TextureUsages::DepthTarget) {
            Some(self.descriptors.allocate(dx::DescriptorHeapType::Dsv, 1))
        } else {
            None
        };

        if let Some(descriptor) = &descriptor {
            self.create_texture_view(descriptor, &raw, &view, &desc);
        }

        if let Some(name) = &desc.name {
            let debug_name = CString::new(name.as_bytes()).expect("failed to create resource name");
            raw.set_debug_object_name(&debug_name)
                .expect("failed to set debug object name");
        }

        DxTexture {
            raw,
            state: Mutex::new(dx::ResourceStates::Common),
            desc,
            flavor: TextureFlavor::Local,
            size,
            descriptor,
            view,
            is_view: false,
        }
    }

    fn create_shared_texture(&self, desc: TextureDesc, heap: dx::Heap) -> DxTexture {
        let raw_desc = map_texture_desc(&desc, self.desc.is_cross_adapter_texture_supported);

        let cross_adapter = raw_desc
            .flags()
            .contains(dx::ResourceFlags::AllowCrossAdapter);

        if cross_adapter {
            let raw_desc = raw_desc.with_layout(dx::TextureLayout::RowMajor);

            let size = self
                .gpu
                .get_copyable_footprints(&raw_desc, 0..1, 0, None, None, None);

            let cross_res = self
                .gpu
                .create_placed_resource(&heap, 0, &raw_desc, dx::ResourceStates::Common, None)
                .expect("failed to create cross texture");

            let view = desc.to_default_view();

            let descriptor = if desc.usage.contains(TextureUsages::RenderTarget) {
                Some(self.descriptors.allocate(dx::DescriptorHeapType::Rtv, 1))
            } else if (desc.usage.contains(TextureUsages::DepthTarget)) {
                Some(self.descriptors.allocate(dx::DescriptorHeapType::Dsv, 1))
            } else {
                None
            };

            if let Some(descriptor) = &descriptor {
                self.create_texture_view(descriptor, &cross_res, &view, &desc);
            }

            if let Some(name) = &desc.name {
                let debug_name =
                    CString::new(name.as_bytes()).expect("failed to create resource name");
                cross_res
                    .set_debug_object_name(&debug_name)
                    .expect("failed to set debug object name");
            }

            DxTexture {
                raw: cross_res,
                state: Mutex::new(dx::ResourceStates::Common),
                desc,
                flavor: TextureFlavor::CrossAdapter { heap },
                size,
                descriptor,
                view,
                is_view: false,
            }
        } else {
            let raw = self
                .gpu
                .create_committed_resource(
                    &dx::HeapProperties::default(),
                    dx::HeapFlags::empty(),
                    &raw_desc,
                    dx::ResourceStates::Common,
                    None,
                )
                .expect("Failed to create buffer");

            let view = desc.to_default_view();

            let descriptor = if desc.usage.contains(TextureUsages::RenderTarget) {
                Some(self.descriptors.allocate(dx::DescriptorHeapType::Rtv, 1))
            } else if desc.usage.contains(TextureUsages::DepthTarget) {
                Some(self.descriptors.allocate(dx::DescriptorHeapType::Dsv, 1))
            } else {
                None
            };

            if let Some(descriptor) = &descriptor {
                self.create_texture_view(descriptor, &raw, &view, &desc);
            }

            let cross_desc = raw_desc
                .with_flags(dx::ResourceFlags::AllowCrossAdapter)
                .with_layout(dx::TextureLayout::RowMajor);

            let size = self
                .gpu
                .get_copyable_footprints(&raw_desc, 0..1, 0, None, None, None);

            let cross_res = self
                .gpu
                .create_placed_resource(&heap, 0, &cross_desc, dx::ResourceStates::Common, None)
                .expect("failed to create cross texture");

            if let Some(name) = &desc.name {
                let debug_name = CString::new(format!("{} Local", name).as_bytes())
                    .expect("failed to create resource name");
                raw.set_debug_object_name(&debug_name)
                    .expect("failed to set debug object name");

                let debug_name = CString::new(format!("{} Cross", name).as_bytes())
                    .expect("failed to create resource name");
                cross_res
                    .set_debug_object_name(&debug_name)
                    .expect("failed to set debug object name");
            }

            DxTexture {
                raw,
                state: Mutex::new(dx::ResourceStates::Common),
                desc,
                flavor: TextureFlavor::Binded {
                    heap,
                    cross: cross_res,
                    cross_state: Mutex::new(dx::ResourceStates::Common),
                },
                size,
                descriptor,
                view,
                is_view: false,
            }
        }
    }

    pub(super) fn create_texture_view(
        &self,
        descriptor: &Descriptor,
        texture: &dx::Resource,
        view: &TextureViewDesc,
        desc: &TextureDesc,
    ) {
        match (view.view_ty, view.ty.unwrap_or(desc.ty)) {
            (TextureViewType::RenderTarget, TextureType::D1) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::RenderTargetViewDesc::texture_1d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                );

                self.gpu
                    .create_render_target_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::RenderTarget, TextureType::D1Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::RenderTargetViewDesc::texture_1d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_render_target_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::RenderTarget, TextureType::D2) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::RenderTargetViewDesc::texture_2d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    0,
                );

                self.gpu
                    .create_render_target_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::RenderTarget, TextureType::D2Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::RenderTargetViewDesc::texture_2d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    0,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_render_target_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::RenderTarget, TextureType::D3) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::RenderTargetViewDesc::texture_3d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_render_target_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::DepthStencil, TextureType::D1) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::DepthStencilViewDesc::texture_1d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                );

                self.gpu
                    .create_depth_stencil_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::DepthStencil, TextureType::D1Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::DepthStencilViewDesc::texture_1d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_depth_stencil_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::DepthStencil, TextureType::D2) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::DepthStencilViewDesc::texture_2d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                );

                self.gpu
                    .create_depth_stencil_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::DepthStencil, TextureType::D2Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::DepthStencilViewDesc::texture_2d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_depth_stencil_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::DepthStencil, TextureType::D3) => {
                panic!("Can not create Depth Stencil View for Volume Texture")
            }
            (TextureViewType::ShaderResource, TextureType::D1) => {
                let mip_start = view.mips.as_ref().map(|m| m.start).unwrap_or(0) as u32;
                let mip_count = view
                    .mips
                    .as_ref()
                    .map(|mips| mips.len())
                    .unwrap_or(desc.mip_levels as usize) as u32;
                let view = dx::ShaderResourceViewDesc::texture_1d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mip_start,
                    mip_count,
                    0.0,
                );

                self.gpu
                    .create_shader_resource_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::ShaderResource, TextureType::D1Array) => {
                let mip_start = view.mips.as_ref().map(|m| m.start).unwrap_or(0) as u32;
                let mip_count = view
                    .mips
                    .as_ref()
                    .map(|mips| mips.len())
                    .unwrap_or(desc.mip_levels as usize) as u32;
                let view = dx::ShaderResourceViewDesc::texture_1d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mip_start,
                    mip_count,
                    0.0,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_shader_resource_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::ShaderResource, TextureType::D2) => {
                let mip_start = view.mips.as_ref().map(|m| m.start).unwrap_or(0) as u32;
                let mip_count = view
                    .mips
                    .as_ref()
                    .map(|mips| mips.len())
                    .unwrap_or(desc.mip_levels as usize) as u32;
                let view = dx::ShaderResourceViewDesc::texture_2d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mip_start,
                    mip_count,
                    0.0,
                    0,
                );

                self.gpu
                    .create_shader_resource_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::ShaderResource, TextureType::D2Array) => {
                let mip_start = view.mips.as_ref().map(|m| m.start).unwrap_or(0) as u32;
                let mip_count = view
                    .mips
                    .as_ref()
                    .map(|mips| mips.len())
                    .unwrap_or(desc.mip_levels as usize) as u32;
                let view = dx::ShaderResourceViewDesc::texture_2d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mip_start,
                    mip_count,
                    0.0,
                    0,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu
                    .create_shader_resource_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::ShaderResource, TextureType::D3) => {
                let mip_start = view.mips.as_ref().map(|m| m.start).unwrap_or(0) as u32;
                let mip_count = view
                    .mips
                    .as_ref()
                    .map(|mips| mips.len())
                    .unwrap_or(desc.mip_levels as usize) as u32;
                let view = dx::ShaderResourceViewDesc::texture_3d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mip_start,
                    mip_count,
                    0.0,
                );

                self.gpu
                    .create_shader_resource_view(Some(texture), Some(&view), descriptor.cpu);
            }
            (TextureViewType::Storage, TextureType::D1) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::UnorderedAccessViewDesc::texture_1d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                );

                self.gpu.create_unordered_access_view(
                    Some(texture),
                    dx::RES_NONE,
                    Some(&view),
                    descriptor.cpu,
                );
            }
            (TextureViewType::Storage, TextureType::D1Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::UnorderedAccessViewDesc::texture_1d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu.create_unordered_access_view(
                    Some(texture),
                    dx::RES_NONE,
                    Some(&view),
                    descriptor.cpu,
                );
            }
            (TextureViewType::Storage, TextureType::D2) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::UnorderedAccessViewDesc::texture_2d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    0,
                );

                self.gpu.create_unordered_access_view(
                    Some(texture),
                    dx::RES_NONE,
                    Some(&view),
                    descriptor.cpu,
                );
            }
            (TextureViewType::Storage, TextureType::D2Array) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::UnorderedAccessViewDesc::texture_2d_array(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    0,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu.create_unordered_access_view(
                    Some(texture),
                    dx::RES_NONE,
                    Some(&view),
                    descriptor.cpu,
                );
            }
            (TextureViewType::Storage, TextureType::D3) => {
                let mips = view.mips.as_ref().map(|m| m.start).unwrap_or(0);
                let view = dx::UnorderedAccessViewDesc::texture_3d(
                    map_format(view.format.unwrap_or(desc.format)),
                    mips as u32,
                    view.array.clone().unwrap_or(0..desc.extent[2]),
                );

                self.gpu.create_unordered_access_view(
                    Some(texture),
                    dx::RES_NONE,
                    Some(&view),
                    descriptor.cpu,
                );
            }
        }
    }
}
