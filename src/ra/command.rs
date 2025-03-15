use crate::rhi::command::RenderCommandQueue;

use super::context::{Context, RenderDevice};

pub trait RenderCommandContext {
    type CommandQueue: RenderCommandQueue;

    fn graphics_queue(&self) -> &Self::CommandQueue;
    fn compute_queue(&self) -> &Self::CommandQueue;
    fn transfer_queue(&self) -> &Self::CommandQueue;
}

impl<D: RenderDevice> RenderCommandContext for Context<D> {
    type CommandQueue = D::CommandQueue;

    fn graphics_queue(&self) -> &Self::CommandQueue {
        &self.graphics_queue
    }

    fn compute_queue(&self) -> &Self::CommandQueue {
        &self.compute_queue
    }

    fn transfer_queue(&self) -> &Self::CommandQueue {
        &self.transfer_queue
    }
}
