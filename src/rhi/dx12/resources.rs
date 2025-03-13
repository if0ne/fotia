use crate::rhi::resources::RenderResourceDevice;

use super::device::DxDevice;

impl RenderResourceDevice for DxDevice {
    type Buffer = ();
    type Texture = ();
    type Sampler = ();

    fn create_buffer(&self, desc: crate::rhi::resources::BufferDesc) -> Self::Buffer {
        todo!()
    }

    fn destroy_buffer(&self, buffer: Self::Buffer) {
        todo!()
    }

    fn create_texture(&self, desc: crate::rhi::resources::TextureDesc) -> Self::Texture {
        todo!()
    }

    fn destroy_texture(&self, texture: Self::Texture) {
        todo!()
    }

    fn create_texture_view(
        &self,
        texture: &Self::Texture,
        desc: crate::rhi::resources::TextureViewDesc,
    ) -> Self::Texture {
        todo!()
    }

    fn open_texture(&self, texture: &Self::Texture, other_gpu: &Self) -> Self::Texture {
        todo!()
    }

    fn create_sampler(&self, desc: crate::rhi::resources::SamplerDesc) -> Self::Sampler {
        todo!()
    }

    fn destroy_sampler(&self, sampler: Self::Sampler) {
        todo!()
    }
}
