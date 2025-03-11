use super::{
    command::{CommandType, GpuEvent, RenderCommandQueue},
    resources::{BufferDesc, SamplerDesc, TextureDesc, TextureViewDesc},
};

pub trait RenderDevice {
    type Buffer;
    type Texture;
    type Sampler;

    type CommandQueue: RenderCommandQueue;
    type Event: GpuEvent;

    fn create_buffer(&self, desc: BufferDesc, init_data: Option<&[u8]>) -> Self::Buffer;
    fn destroy_buffer(&self, buffer: Self::Buffer);

    fn create_texture(&self, desc: TextureDesc, init_data: Option<&[u8]>) -> Self::Texture;
    fn destroy_texture(&self, buffer: Self::Texture);

    fn create_texture_view(&self, texture: &Self::Texture, desc: TextureViewDesc) -> Self::Texture;

    fn open_texture(&self, texture: &Self::Texture, other_gpu: &Self) -> Self::Texture;

    fn create_sampler(&self, desc: SamplerDesc) -> Self::Sampler;
    fn destroy_sampler(&self, sampler: Self::Sampler);

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue;

    fn create_event(&self, shared: bool) -> Self::Event;
    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event;
}
