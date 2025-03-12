use crate::rhi::{
    command::{CommandType, RenderCommandDevice},
    resources::RenderResourceDevice,
};

use super::resources::ResourceMapper;

pub struct Context<D: RenderResourceDevice + RenderCommandDevice> {
    pub(super) gpu: D,

    pub(super) graphics_queue: D::CommandQueue,
    pub(super) compute_queue: D::CommandQueue,
    pub(super) transfer_queue: D::CommandQueue,

    pub(super) mapper: ResourceMapper<D>,
}

impl<D: RenderResourceDevice + RenderCommandDevice> Context<D> {
    pub fn new(gpu: D) -> Self {
        let graphics_queue = gpu.create_command_queue(CommandType::Graphics, None);
        let compute_queue = gpu.create_command_queue(CommandType::Compute, None);
        let transfer_queue = gpu.create_command_queue(CommandType::Transfer, None);

        Self {
            gpu,
            graphics_queue,
            compute_queue,
            transfer_queue,
            mapper: ResourceMapper::default(),
        }
    }
}

pub struct ContextDual<D: RenderResourceDevice + RenderCommandDevice> {
    primary: Context<D>,
    secondary: Context<D>,
}

impl<D: RenderResourceDevice + RenderCommandDevice> ContextDual<D> {
    pub fn new(primary: Context<D>, secondary: Context<D>) -> Self {
        Self { primary, secondary }
    }

    pub fn call(&self, func: impl Fn(&Context<D>)) {
        func(&self.primary);
        func(&self.secondary);
    }

    pub fn call_primary(&self, func: impl Fn(&Context<D>)) {
        func(&self.primary);
    }

    pub fn call_secondary(&self, func: impl Fn(&Context<D>)) {
        func(&self.secondary);
    }
}
