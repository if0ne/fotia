use crate::rhi::{
    command::{CommandType, RenderCommandDevice, RenderCommandQueue},
    resources::RenderResourceDevice,
    shader::RenderShaderDevice,
};

use super::resources::ResourceMapper;

pub struct Context<D: RenderResourceDevice + RenderCommandDevice + RenderShaderDevice> {
    pub(super) gpu: D,

    pub(super) graphics_queue: D::CommandQueue,
    pub(super) compute_queue: D::CommandQueue,
    pub(super) transfer_queue: D::CommandQueue,

    pub(super) uploader: D::ResourceUploader,

    pub(super) mapper: ResourceMapper<D>,
}

impl<D: RenderResourceDevice + RenderCommandDevice + RenderShaderDevice> Context<D> {
    pub fn new(gpu: D) -> Self {
        let graphics_queue = gpu.create_command_queue(CommandType::Graphics, None);
        let compute_queue = gpu.create_command_queue(CommandType::Compute, None);
        let transfer_queue = gpu.create_command_queue(CommandType::Transfer, None);

        let uploader = gpu.create_resource_uploader();

        Self {
            gpu,
            graphics_queue,
            compute_queue,
            transfer_queue,
            uploader,
            mapper: ResourceMapper::default(),
        }
    }

    pub fn wait_idle(&self) {
        self.graphics_queue.wait_idle();
        self.compute_queue.wait_idle();
        self.transfer_queue.wait_idle();
        self.uploader.wait_idle();
    }
}

pub struct ContextDual<D: RenderResourceDevice + RenderCommandDevice + RenderShaderDevice> {
    pub primary: Context<D>,
    pub secondary: Context<D>,
}

impl<D: RenderResourceDevice + RenderCommandDevice + RenderShaderDevice> ContextDual<D> {
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
