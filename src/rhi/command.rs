use super::resources::RenderResourceDevice;

pub type SyncPoint = u64;

pub trait RenderCommandDevice: RenderResourceDevice {
    type ResourceUploader: RenderResourceUploader<Self>;
    type CommandQueue: RenderCommandQueue;
    type Event;

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue;
    fn create_resource_uploader(&self) -> Self::ResourceUploader;

    fn create_event(&self, shared: bool) -> Self::Event;
    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event;

    fn wait_idle(&self);
}

pub trait RenderCommandQueue {
    type Event;
    type CommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer;
    fn enqueue(&self, cmd_buffer: Self::CommandBuffer);
    fn commit(&self, cmd_buffer: Self::CommandBuffer);
    fn submit(&self) -> SyncPoint;

    fn signal_event(&self, event: &Self::Event) -> SyncPoint;
    fn wait_event(&self, event: &Self::Event);

    fn wait_on_cpu(&self, value: SyncPoint);
    fn wait_until_complete(&self);
    fn wait_idle(&self);
}

pub trait RenderResourceUploader<D: RenderResourceDevice>:
    RenderCommandQueue<CommandBuffer: IoCommandBuffer<D>>
{
}

pub trait IoCommandBuffer<D: RenderResourceDevice> {
    fn load_to_buffer(&self, buffer: &D::Buffer, data: &'_ [u8]);
    fn load_to_texture(&self, texture: &D::Texture, data: &'_ [u8]);
}

pub trait RenderCommandBuffer {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandType {
    Graphics,
    Compute,
    Transfer,
}

pub trait GpuEvent {
    fn wait(&self, value: SyncPoint) -> bool;
    fn increment(&self) -> SyncPoint;
    fn get_completed_value(&self) -> SyncPoint;
    fn get_goal(&self) -> SyncPoint;
}
