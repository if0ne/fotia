use super::{
    resources::RenderResourceDevice,
    types::{IndexType, Scissor, Viewport},
};

pub type SyncPoint = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandType {
    Graphics,
    Compute,
    Transfer,
}

pub trait RenderCommandDevice: RenderResourceDevice {
    type ResourceUploader: RenderResourceUploader<Device = Self, CommandBuffer: IoCommandBuffer<Device = Self>>;
    type CommandQueue: RenderCommandQueue<Device = Self, CommandBuffer: RenderCommandBuffer>;
    type Event;

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue;
    fn create_resource_uploader(&self) -> Self::ResourceUploader;

    fn create_event(&self, shared: bool) -> Self::Event;
    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event;

    fn wait_idle(&self);
}

pub trait RenderCommandQueue {
    type Device;
    type Event;
    type CommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer;
    fn enqueue(&self, cmd_buffer: Self::CommandBuffer);
    fn commit(&self, cmd_buffer: Self::CommandBuffer);
    fn submit(&self, device: &Self::Device) -> SyncPoint;

    fn signal_event(&self, event: &Self::Event) -> SyncPoint;
    fn wait_event(&self, event: &Self::Event);

    fn wait_on_cpu(&self, value: SyncPoint);
    fn wait_until_complete(&self);
    fn wait_idle(&self);
}

pub trait RenderResourceUploader: RenderCommandQueue<CommandBuffer: IoCommandBuffer> {
    fn flush(&self, device: &Self::Device);
}

pub trait IoCommandBuffer {
    type Device: RenderResourceDevice;

    fn load_to_buffer(
        &mut self,
        device: &Self::Device,
        buffer: &<Self::Device as RenderResourceDevice>::Buffer,
        data: &'_ [u8],
    );
    fn load_to_texture(
        &mut self,
        device: &Self::Device,
        texture: &<Self::Device as RenderResourceDevice>::Texture,
        data: &'_ [u8],
    );
}

pub trait RenderCommandBuffer {
    type Device: RenderResourceDevice;
    type RenderEncoder<'a>
    where
        Self: 'a;

    fn ty(&self) -> CommandType;

    fn begin(&mut self, device: &Self::Device);

    fn render(
        &mut self,
        targets: &[&<Self::Device as RenderResourceDevice>::Texture],
        depth: Option<&<Self::Device as RenderResourceDevice>::Texture>,
    ) -> Self::RenderEncoder<'_>;
}

pub trait GpuEvent {
    fn wait(&self, value: SyncPoint) -> bool;
    fn increment(&self) -> SyncPoint;
    fn get_completed_value(&self) -> SyncPoint;
    fn get_goal(&self) -> SyncPoint;
}

pub trait RenderEncoder {
    type Buffer;
    type RasterPipeline;
    type ShaderArgument;

    fn set_viewport(&mut self, viewport: Viewport);
    fn set_scissor(&mut self, scissor: Scissor);

    fn set_raster_pipeline(&mut self, pipeline: &Self::RasterPipeline);
    fn bind_shader_argument(&mut self, argument: &Self::ShaderArgument, dynamic_offset: u64);

    fn bind_vertex_buffer(&mut self, buffer: &Self::Buffer, slot: usize);
    fn bind_index_buffer(&mut self, buffer: &Self::Buffer, ty: IndexType);

    fn draw(&mut self, count: u32, start_vertex: u32);
    fn draw_indexed(&mut self, count: u32, start_index: u32, base_index: u32);
}
