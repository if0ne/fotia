use crate::rhi::command::{
    CommandType, RenderCommandBuffer, RenderCommandDevice, RenderCommandQueue, SyncPoint,
};

use super::context::{Context, RenderDevice};

type CommandBuffer<D> =
    <<D as RenderCommandDevice>::CommandQueue as RenderCommandQueue>::CommandBuffer;

pub trait RenderCommandContext<D: RenderDevice> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<CommandBuffer<D>>;
    fn enqueue(&self, cmd: CommandEncoder<CommandBuffer<D>>);
    fn commit(&self, cmd: CommandEncoder<CommandBuffer<D>>);
    fn submit(&self, ty: CommandType) -> SyncPoint;
}

impl<D: RenderDevice> RenderCommandContext<D> for Context<D> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<CommandBuffer<D>> {
        match ty {
            CommandType::Graphics => CommandEncoder {
                raw: self.graphics_queue.create_command_buffer(),
            },
            CommandType::Compute => CommandEncoder {
                raw: self.compute_queue.create_command_buffer(),
            },
            CommandType::Transfer => CommandEncoder {
                raw: self.transfer_queue.create_command_buffer(),
            },
        }
    }

    fn enqueue(&self, cmd: CommandEncoder<CommandBuffer<D>>) {
        match cmd.raw.ty() {
            CommandType::Graphics => self.graphics_queue.enqueue(cmd.raw),
            CommandType::Compute => self.compute_queue.enqueue(cmd.raw),
            CommandType::Transfer => self.transfer_queue.enqueue(cmd.raw),
        }
    }

    fn commit(&self, cmd: CommandEncoder<CommandBuffer<D>>) {
        match cmd.raw.ty() {
            CommandType::Graphics => self.graphics_queue.commit(cmd.raw),
            CommandType::Compute => self.compute_queue.commit(cmd.raw),
            CommandType::Transfer => self.transfer_queue.commit(cmd.raw),
        }
    }

    fn submit(&self, ty: CommandType) -> SyncPoint {
        match ty {
            CommandType::Graphics => self.graphics_queue.submit(&self.gpu),
            CommandType::Compute => self.compute_queue.submit(&self.gpu),
            CommandType::Transfer => self.transfer_queue.submit(&self.gpu),
        }
    }
}

pub struct CommandEncoder<C: RenderCommandBuffer> {
    pub(super) raw: C,
}
