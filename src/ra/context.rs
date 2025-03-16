use std::sync::Arc;

use crate::rhi::{
    command::{
        CommandType, RenderCommandBuffer, RenderCommandDevice, RenderCommandQueue, RenderEncoder,
    },
    resources::RenderResourceDevice,
    shader::RenderShaderDevice,
    swapchain::{RenderSwapchainDevice, Surface},
};

use super::{command::CommandQueue, resources::ResourceMapper};

pub trait RenderDevice:
    RenderResourceDevice
    + RenderCommandDevice<
        CommandQueue: for<'a> RenderCommandQueue<
            CommandBuffer: RenderCommandBuffer<
                Device = Self,
                RenderEncoder<'a>: RenderEncoder<
                    Buffer = Self::Buffer,
                    Texture = Self::Texture,
                    ShaderArgument = Self::ShaderArgument,
                    RasterPipeline = Self::RasterPipeline,
                >,
            >,
            Event = Self::Event,
        >,
    > + RenderShaderDevice
    + RenderSwapchainDevice<Swapchain: Surface<Texture = Self::Texture>, Queue = Self::CommandQueue>
{
}

impl<T> RenderDevice for T where
    T: RenderResourceDevice
        + RenderCommandDevice<
            CommandQueue: for<'a> RenderCommandQueue<
                CommandBuffer: RenderCommandBuffer<
                    Device = T,
                    RenderEncoder<'a>: RenderEncoder<
                        Buffer = T::Buffer,
                        Texture = T::Texture,
                        ShaderArgument = T::ShaderArgument,
                        RasterPipeline = T::RasterPipeline,
                    >,
                >,
                Event = T::Event,
            >,
        > + RenderShaderDevice
        + RenderSwapchainDevice<Swapchain: Surface<Texture = Self::Texture>, Queue = T::CommandQueue>
{
}

pub struct Context<D: RenderDevice> {
    pub(super) gpu: D,

    pub(super) graphics_queue: CommandQueue<D>,
    pub(super) compute_queue: CommandQueue<D>,
    pub(super) transfer_queue: CommandQueue<D>,

    pub(super) uploader: D::ResourceUploader,

    pub(super) mapper: Arc<ResourceMapper<D>>,
}

impl<D: RenderDevice> Context<D> {
    pub fn new(gpu: D) -> Self {
        let mapper = Arc::new(ResourceMapper::default());

        let graphics_queue = CommandQueue::new(
            gpu.create_command_queue(CommandType::Graphics, None),
            Arc::clone(&mapper),
        );
        let compute_queue = CommandQueue::new(
            gpu.create_command_queue(CommandType::Compute, None),
            Arc::clone(&mapper),
        );
        let transfer_queue = CommandQueue::new(
            gpu.create_command_queue(CommandType::Transfer, None),
            Arc::clone(&mapper),
        );

        let uploader = gpu.create_resource_uploader();

        Self {
            gpu,
            graphics_queue,
            compute_queue,
            transfer_queue,
            uploader,
            mapper,
        }
    }

    pub fn wait_idle(&self) {
        self.graphics_queue.wait_idle();
        self.compute_queue.wait_idle();
        self.transfer_queue.wait_idle();
        self.uploader.wait_idle();
    }
}

pub struct ContextDual<D: RenderDevice> {
    pub primary: Context<D>,
    pub secondary: Context<D>,
}

impl<D: RenderDevice> ContextDual<D> {
    pub fn new(primary: Context<D>, secondary: Context<D>) -> Self {
        Self { primary, secondary }
    }

    pub fn call(&self, func: impl Fn(&Context<D>)) {
        func(&self.primary);
        func(&self.secondary);
    }

    pub fn call_primary(&self, mut func: impl FnMut(&Context<D>)) {
        func(&self.primary);
    }

    pub fn call_secondary(&self, mut func: impl FnMut(&Context<D>)) {
        func(&self.secondary);
    }
}
