use std::sync::Arc;

use crate::{
    collections::handle::Handle,
    rhi::{
        command::{
            CommandType, RenderCommandBuffer, RenderCommandDevice, RenderCommandQueue,
            RenderEncoder as _, SyncPoint,
        },
        types::{IndexType, Scissor, Viewport},
    },
};

use super::{
    context::{Context, RenderDevice},
    resources::{Buffer, ResourceMapper, Texture},
    shader::{RasterPipeline, ShaderArgument},
};

type CommandBuffer<D> =
    <<D as RenderCommandDevice>::CommandQueue as RenderCommandQueue>::CommandBuffer;

type RenderEncoderType<'a, D> = <CommandBuffer<D> as RenderCommandBuffer>::RenderEncoder<'a>;

pub struct CommandQueue<D: RenderDevice> {
    pub(super) raw: D::CommandQueue,
    pub(super) mapper: Arc<ResourceMapper<D>>,
}

impl<D: RenderDevice> CommandQueue<D> {
    pub(super) fn new(raw: D::CommandQueue, mapper: Arc<ResourceMapper<D>>) -> Self {
        Self { raw, mapper }
    }
}

impl<D: RenderDevice> RenderCommandQueue for CommandQueue<D> {
    type Device = D;
    type Event = <D::CommandQueue as RenderCommandQueue>::Event;
    type CommandBuffer = CommandEncoder<D>;

    fn create_command_buffer(&self) -> Self::CommandBuffer {
        let raw = self.raw.create_command_buffer();
        Self::CommandBuffer::new(raw, Arc::clone(&self.mapper))
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.raw.enqueue(cmd_buffer.raw);
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        self.raw.commit(cmd_buffer.raw);
    }

    fn submit(&self, device: &Self::Device) -> SyncPoint {
        self.raw.submit(device)
    }

    fn signal_event(&self, event: &Self::Event) -> SyncPoint {
        self.raw.signal_event(event)
    }

    fn wait_event(&self, event: &Self::Event) {
        self.raw.wait_event(event)
    }

    fn wait_on_cpu(&self, value: SyncPoint) {
        self.raw.wait_on_cpu(value);
    }

    fn wait_until_complete(&self) {
        self.raw.wait_until_complete();
    }

    fn wait_idle(&self) {
        self.raw.wait_idle();
    }
}

pub struct CommandEncoder<D: RenderDevice> {
    pub(super) raw: CommandBuffer<D>,
    pub(super) mapper: Arc<ResourceMapper<D>>,
}

impl<D: RenderDevice> CommandEncoder<D> {
    pub(super) fn new(raw: CommandBuffer<D>, mapper: Arc<ResourceMapper<D>>) -> Self {
        Self { raw, mapper }
    }
}

pub trait RenderCommandContext<D: RenderDevice> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<D>;
    fn enqueue(&self, cmd: CommandEncoder<D>);
    fn commit(&self, cmd: CommandEncoder<D>);
    fn submit(&self, ty: CommandType) -> SyncPoint;
}

impl<D: RenderDevice> RenderCommandContext<D> for Context<D> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<D> {
        match ty {
            CommandType::Graphics => self.graphics_queue.create_command_buffer(),
            CommandType::Compute => self.compute_queue.create_command_buffer(),
            CommandType::Transfer => self.transfer_queue.create_command_buffer(),
        }
    }

    fn enqueue(&self, cmd: CommandEncoder<D>) {
        match cmd.raw.ty() {
            CommandType::Graphics => self.graphics_queue.enqueue(cmd),
            CommandType::Compute => self.compute_queue.enqueue(cmd),
            CommandType::Transfer => self.transfer_queue.enqueue(cmd),
        }
    }

    fn commit(&self, cmd: CommandEncoder<D>) {
        match cmd.raw.ty() {
            CommandType::Graphics => self.graphics_queue.commit(cmd),
            CommandType::Compute => self.compute_queue.commit(cmd),
            CommandType::Transfer => self.transfer_queue.commit(cmd),
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

pub trait RenderCommandEncoder<D: RenderDevice> {
    type RenderEncoder<'a>
    where
        Self: 'a;

    fn begin(&mut self, ctx: &Context<D>);

    fn render(
        &mut self,
        targets: &[Handle<Texture>],
        depth: Option<Handle<Texture>>,
    ) -> Self::RenderEncoder<'_>;
}

impl<D: RenderDevice> RenderCommandEncoder<D> for CommandEncoder<D> {
    type RenderEncoder<'a>
        = RenderEncoderImpl<'a, D>
    where
        D: 'a;

    fn begin(&mut self, ctx: &Context<D>) {
        self.raw.begin(&ctx.gpu);
    }

    fn render(
        &mut self,
        targets: &[Handle<Texture>],
        depth: Option<Handle<Texture>>,
    ) -> Self::RenderEncoder<'_> {
        let guard = self.mapper.textures.lock();
        let targets = targets
            .iter()
            .map(|h| guard.get(*h).expect("failed to get texture"))
            .collect::<Vec<_>>();
        let depth = depth.map(|h| guard.get(h).expect("failed to get texture"));

        let raw = self.raw.render(&targets, depth);

        Self::RenderEncoder {
            raw,
            mapper: &self.mapper,
        }
    }
}

pub struct RenderEncoderImpl<'a, D: RenderDevice> {
    pub(super) raw: RenderEncoderType<'a, D>,
    pub(super) mapper: &'a ResourceMapper<D>,
}

pub trait RenderEncoder {
    fn set_viewport(&mut self, viewport: Viewport);
    fn set_scissor(&mut self, scissor: Scissor);

    fn set_render_pipeline(&mut self, pipeline: Handle<RasterPipeline>);
    fn bind_shader_argument(&mut self, argument: Handle<ShaderArgument>, dynamic_offset: u64);

    fn bind_vertex_buffer(&mut self, buffer: Handle<Buffer>, slot: usize);
    fn bind_index_buffer(&mut self, buffer: Handle<Buffer>, ty: IndexType);

    fn draw(&mut self, count: u32, start_vertex: u32);
    fn draw_indexed(&mut self, count: u32, start_index: u32, base_index: u32);
}

impl<'a, D: RenderDevice> RenderEncoder for RenderEncoderImpl<'a, D> {
    fn set_viewport(&mut self, viewport: Viewport) {
        self.raw.set_viewport(viewport);
    }

    fn set_scissor(&mut self, scissor: Scissor) {
        self.raw.set_scissor(scissor);
    }

    fn set_render_pipeline(&mut self, pipeline: Handle<RasterPipeline>) {
        let guard = self.mapper.raster_pipelines.lock();
        let pipeline = guard.get(pipeline).expect("failed to get pipeline");

        self.raw.set_raster_pipeline(pipeline);
    }

    fn bind_shader_argument(&mut self, argument: Handle<ShaderArgument>, dynamic_offset: u64) {
        let guard = self.mapper.shader_arguments.lock();
        let argument = guard.get(argument).expect("failed to get shader argument");

        self.raw.bind_shader_argument(argument, dynamic_offset);
    }

    fn bind_vertex_buffer(&mut self, buffer: Handle<Buffer>, slot: usize) {
        let guard = self.mapper.buffers.lock();
        let buffer = guard.get(buffer).expect("failed to get buffer");

        self.raw.bind_vertex_buffer(buffer, slot);
    }

    fn bind_index_buffer(&mut self, buffer: Handle<Buffer>, ty: IndexType) {
        let guard = self.mapper.buffers.lock();
        let buffer = guard.get(buffer).expect("failed to get buffer");

        self.raw.bind_index_buffer(buffer, ty);
    }

    fn draw(&mut self, count: u32, start_vertex: u32) {
        self.raw.draw(count, start_vertex);
    }

    fn draw_indexed(&mut self, count: u32, start_index: u32, base_index: u32) {
        self.raw.draw_indexed(count, start_index, base_index);
    }
}
