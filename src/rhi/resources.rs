use std::{borrow::Cow, ops::Range};

use super::types::{AddressMode, Filter, Format};

pub trait RenderResourceDevice: Sized {
    type Buffer;
    type Texture;
    type Sampler;

    fn create_buffer(&self, desc: BufferDesc) -> Self::Buffer;
    fn destroy_buffer(&self, buffer: Self::Buffer);

    fn create_texture(&self, desc: TextureDesc) -> Self::Texture;
    fn destroy_texture(&self, buffer: Self::Texture);

    fn create_texture_view(&self, texture: &Self::Texture, desc: TextureViewDesc) -> Self::Texture;

    fn open_texture(&self, texture: &Self::Texture, other_gpu: &Self) -> Self::Texture;

    fn create_sampler(&self, desc: SamplerDesc) -> Self::Sampler;
    fn destroy_sampler(&self, sampler: Self::Sampler);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryLocation {
    CpuToGpu,
    GpuToGpu,
    GpuToCpu,
    SharedGpu,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    pub name: Option<Cow<'static, str>>,
    pub size: usize,
    pub stride: usize,
    pub usage: BufferUsages,
    pub memory_location: MemoryLocation,
}

impl BufferDesc {
    pub fn cpu_to_gpu(size: usize, usage: BufferUsages) -> Self {
        Self {
            name: None,
            size,
            stride: 0,
            usage,
            memory_location: MemoryLocation::CpuToGpu,
        }
    }

    pub fn gpu_to_gpu(size: usize, usage: BufferUsages) -> Self {
        Self {
            name: None,
            size,
            stride: 0,
            usage,
            memory_location: MemoryLocation::GpuToGpu,
        }
    }

    pub fn gpu_to_cpu(size: usize, usage: BufferUsages) -> Self {
        Self {
            name: None,
            size,
            stride: 0,
            usage,
            memory_location: MemoryLocation::GpuToCpu,
        }
    }

    pub fn with_stride(mut self, stride: usize) -> Self {
        self.stride = stride;
        self
    }

    pub fn with_name(mut self, name: Cow<'static, str>) -> Self {
        self.name = Some(name);
        self
    }
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct BufferUsages: u32 {
        const Copy = 1 << 0;
        const Uniform = 1 << 1;
        const Vertex = 1 << 2;
        const Index = 1 << 3;
        const Storage = 1 << 4;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    pub name: Option<Cow<'static, str>>,
    pub ty: TextureType,
    pub extent: [u32; 3],
    pub mip_levels: u16,
    pub format: Format,
    pub usage: TextureUsages,
}

impl TextureDesc {
    pub fn new_1d(extent: u32, format: Format, usage: TextureUsages) -> Self {
        Self {
            name: None,
            ty: TextureType::D1,
            extent: [extent, 0, 0],
            mip_levels: 1,
            format,
            usage,
        }
    }

    pub fn new_1d_array(extent: u32, size: u32, format: Format, usage: TextureUsages) -> Self {
        Self {
            name: None,
            ty: TextureType::D1Array,
            extent: [extent, 0, size],
            mip_levels: 1,
            format,
            usage,
        }
    }

    pub fn new_2d(extent: [u32; 2], format: Format, usage: TextureUsages) -> Self {
        Self {
            name: None,
            ty: TextureType::D2,
            extent: [extent[0], extent[1], 0],
            mip_levels: 1,
            format,
            usage,
        }
    }

    pub fn new_2d_array(extent: [u32; 2], size: u32, format: Format, usage: TextureUsages) -> Self {
        Self {
            name: None,
            ty: TextureType::D2Array,
            extent: [extent[0], extent[1], size],
            mip_levels: 1,
            format,
            usage,
        }
    }

    pub fn new_3d(extent: [u32; 3], format: Format, usage: TextureUsages) -> Self {
        Self {
            name: None,
            ty: TextureType::D3,
            extent,
            mip_levels: 1,
            format,
            usage,
        }
    }

    pub fn with_name(mut self, name: Cow<'static, str>) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_mip_levels(mut self, mip_levels: u16) -> Self {
        self.mip_levels = mip_levels;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureType {
    D1,
    D1Array,
    D2,
    D2Array,
    D3,
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct TextureUsages: u32 {
        const Copy = 1 << 0;
        const Resource = 1 << 1;
        const RenderTarget = 1 << 2;
        const DepthTarget = 1 << 3;
        const Storage = 1 << 4;
        const Shared = 1 << 5;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureViewDesc {
    pub format: Option<Format>,
    pub ty: Option<TextureType>,
    pub mips: Option<Range<u16>>,
    pub array: Option<Range<u32>>,
}

impl TextureViewDesc {
    pub fn with_format(mut self, format: Format) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_type(mut self, ty: TextureType) -> Self {
        self.ty = Some(ty);
        self
    }

    pub fn with_mips(mut self, mips: Range<u16>) -> Self {
        self.mips = Some(mips);
        self
    }

    pub fn with_array(mut self, array: Range<u32>) -> Self {
        self.array = Some(array);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub filter: Filter,
    pub address_mode: AddressMode,
}

impl SamplerDesc {
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_address_mode(mut self, address_mode: AddressMode) -> Self {
        self.address_mode = address_mode;
        self
    }
}
