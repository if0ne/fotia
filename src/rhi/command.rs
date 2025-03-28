use std::borrow::Cow;

use super::{
    resources::RenderResourceDevice,
    types::{GeomTopology, IndexType, ResourceState, Scissor, Timings, Viewport},
};

pub type SyncPoint = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandType {
    Graphics,
    Compute,
    Transfer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Subresource {
    Local(Option<u32>),
    Shared,
}

#[derive(Debug)]
pub enum Barrier<'a, D: RenderResourceDevice> {
    Buffer(&'a D::Buffer, ResourceState),
    Texture(&'a D::Texture, ResourceState, Subresource),
}

pub trait RenderCommandDevice: RenderResourceDevice {
    type ResourceUploader: RenderResourceUploader<Device = Self, CommandBuffer: IoCommandBuffer<Device = Self>>
        + Sync;
    type CommandQueue: RenderCommandQueue<Device = Self, CommandBuffer: RenderCommandBuffer> + Sync;
    type Event;

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue;
    fn create_resource_uploader(&self) -> Self::ResourceUploader;

    fn create_event(&self, shared: bool) -> Self::Event;
    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event;

    fn wait_idle(&self);
}

pub trait RenderCommandQueue: Send + Sync {
    type Device;
    type Event;
    type CommandBuffer;

    fn ty(&self) -> CommandType;
    fn frequency(&self) -> f64;

    fn create_command_buffer(&self, device: &Self::Device) -> Self::CommandBuffer;
    fn enqueue(&self, cmd_buffer: Self::CommandBuffer);
    fn commit(&self, cmd_buffer: Self::CommandBuffer);
    fn submit(&self, device: &Self::Device) -> SyncPoint;

    fn signal_event(&self, event: &Self::Event) -> SyncPoint;
    fn wait_event(&self, event: &Self::Event);

    fn wait_on_cpu(&self, value: SyncPoint);
    fn wait_until_complete(&self);
    fn wait_idle(&self);

    fn is_ready(&self) -> bool;
    fn is_ready_for(&self, v: u64) -> bool;
}

pub trait RenderResourceUploader: RenderCommandQueue<CommandBuffer: IoCommandBuffer> {
    fn flush(&self, device: &Self::Device);
}

pub trait IoCommandBuffer {
    type Device: RenderResourceDevice;

    fn load_to_buffer(
        &mut self,
        device: &Self::Device,
        buffer: &mut <Self::Device as RenderResourceDevice>::Buffer,
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

    type TransferEncoder<'a>
    where
        Self: 'a;

    fn ty(&self) -> CommandType;

    fn begin(&mut self, device: &Self::Device) -> Option<Timings>;

    fn set_barriers<'a>(&self, barriers: impl IntoIterator<Item = Barrier<'a, Self::Device>>);

    fn render<'a>(
        &mut self,
        label: Cow<'static, str>,
        targets: impl IntoIterator<Item = &'a <Self::Device as RenderResourceDevice>::Texture>,
        depth: Option<&<Self::Device as RenderResourceDevice>::Texture>,
    ) -> Self::RenderEncoder<'_>;

    fn transfer<'a>(&mut self, label: Cow<'static, str>) -> Self::TransferEncoder<'_>;

    fn resolve_timestamp_data(&mut self) -> std::ops::Range<usize>;
}

pub trait GpuEvent {
    fn wait(&self, value: SyncPoint) -> bool;
    fn increment(&self) -> SyncPoint;
    fn get_completed_value(&self) -> SyncPoint;
    fn get_goal(&self) -> SyncPoint;
}

pub trait RenderEncoder {
    type Buffer;
    type Texture;
    type RasterPipeline;
    type ShaderArgument;

    fn clear_rt(&self, texture: &Self::Texture, color: Option<[f32; 4]>);
    fn clear_depth(&self, texture: &Self::Texture, depth: Option<f32>);

    fn set_viewport(&self, viewport: Viewport);
    fn set_scissor(&self, scissor: Scissor);

    fn set_topology(&self, topology: GeomTopology);

    fn set_raster_pipeline(&mut self, pipeline: &Self::RasterPipeline);
    fn bind_shader_argument(
        &self,
        set: u32,
        argument: &Self::ShaderArgument,
        dynamic_offset: usize,
    );

    fn bind_vertex_buffer(&self, buffer: &Self::Buffer, slot: usize);
    fn bind_index_buffer(&self, buffer: &Self::Buffer, ty: IndexType);

    fn draw(&self, count: u32, start_vertex: u32);
    fn draw_indexed(&self, count: u32, start_index: u32, base_index: u32);
}

pub trait TransferEncoder {
    type Texture;

    fn pull_texture(&self, texture: &Self::Texture);
    fn push_texture(&self, texture: &Self::Texture);
}
