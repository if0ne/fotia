pub type SyncPoint = u64;

pub trait RenderCommandDevice {
    type CommandQueue: RenderCommandQueue<Event = Self::Event>;
    type Event: GpuEvent;

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue;

    fn create_event(&self, shared: bool) -> Self::Event;
    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event;
}

pub trait RenderCommandQueue {
    type Event: GpuEvent;
    type CommandBuffer: RenderCommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer;
    fn enqueue(&self, cmd_buffer: Self::CommandBuffer);
    fn commit(&self, cmd_buffer: Self::CommandBuffer);
    fn submit(&self) -> SyncPoint;

    fn signal_event(&self, event: Self::Event, value: SyncPoint);
    fn wait_event(&self, event: Self::Event, value: SyncPoint);

    fn wait_on_cpu(&self, value: SyncPoint);
    fn wait_until_complete(&self);
    fn wait_idle(&self);
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
