use crate::rhi::{
    command::{RenderCommandDevice, RenderCommandQueue},
    resources::RenderResourceDevice,
    shader::RenderShaderDevice,
};

use super::context::Context;

pub trait RenderCommandContext {
    type CommandQueue: RenderCommandQueue;

    fn graphics_queue(&self) -> &Self::CommandQueue;
    fn compute_queue(&self) -> &Self::CommandQueue;
    fn transfer_queue(&self) -> &Self::CommandQueue;
}

impl<D: RenderCommandDevice + RenderResourceDevice + RenderShaderDevice> RenderCommandContext
    for Context<D>
{
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
