use std::{collections::VecDeque, ops::Range, sync::Arc};

use parking_lot::Mutex;

use crate::{
    collections::handle::Handle,
    rhi::{
        self,
        command::{
            CommandBufferState, CommandType, RenderCommandBuffer, RenderCommandDevice,
            RenderCommandQueue, RenderEncoder as _, SyncPoint,
        },
        resources::QueryHeap,
        types::{IndexType, ResourceState, Scissor, Viewport},
    },
};

use super::{
    Timings,
    context::{Context, RenderDevice},
    resources::{Buffer, ResourceMapper, Texture},
    shader::{RasterPipeline, ShaderArgument},
};

pub(super) const QUERY_SIZE: usize = 64;

type CommandBuffer<D> =
    <<D as RenderCommandDevice>::CommandQueue as RenderCommandQueue>::CommandBuffer;

type RenderEncoderType<'a, D> = <CommandBuffer<D> as RenderCommandBuffer>::RenderEncoder<'a>;

pub struct CommandQueue<D: RenderDevice> {
    pub(super) raw: D::CommandQueue,
    pub(super) mapper: Arc<ResourceMapper<D>>,
    pub(super) timestamp_queries: Mutex<VecDeque<(D::TimestampQuery, Option<Range<usize>>)>>,
}

impl<D: RenderDevice> CommandQueue<D> {
    pub(super) fn new(
        raw: D::CommandQueue,
        mapper: Arc<ResourceMapper<D>>,
        timestamp_queries: VecDeque<D::TimestampQuery>,
    ) -> Self {
        Self {
            raw,
            mapper,
            timestamp_queries: Mutex::new(
                timestamp_queries.into_iter().map(|q| (q, None)).collect(),
            ),
        }
    }
}

impl<D: RenderDevice> RenderCommandQueue for CommandQueue<D> {
    type Device = D;
    type Event = <D::CommandQueue as RenderCommandQueue>::Event;
    type CommandBuffer = CommandEncoder<D>;

    fn ty(&self) -> CommandType {
        self.raw.ty()
    }

    fn frequency(&self) -> f64 {
        self.raw.frequency()
    }

    fn create_command_buffer(
        &self,
        device: &Self::Device,
    ) -> CommandBufferState<Self::CommandBuffer> {
        match self.raw.create_command_buffer(&device) {
            CommandBufferState::New(cmd) => {
                let (query, range) = self
                    .timestamp_queries
                    .lock()
                    .pop_front()
                    .expect("wrong count of timestamp heaps");
                CommandBufferState::New(Self::CommandBuffer::new(
                    cmd,
                    Arc::clone(&self.mapper),
                    query,
                    range,
                    self.frequency(),
                ))
            }
            CommandBufferState::Stashed(cmd) => {
                let (query, range) = self
                    .timestamp_queries
                    .lock()
                    .pop_front()
                    .expect("wrong count of timestamp heaps");
                CommandBufferState::Stashed(Self::CommandBuffer::new(
                    cmd,
                    Arc::clone(&self.mapper),
                    query,
                    range,
                    self.frequency(),
                ))
            }
            CommandBufferState::Created(cmd) => {
                let query = device.create_timestamp_query(self.ty(), QUERY_SIZE);
                CommandBufferState::Created(Self::CommandBuffer::new(
                    cmd,
                    Arc::clone(&self.mapper),
                    query,
                    None,
                    self.frequency(),
                ))
            }
        }
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.timestamp_queries
            .lock()
            .push_front((cmd_buffer.query, cmd_buffer.range));
        self.raw.enqueue(cmd_buffer.raw);
    }

    fn commit(&self, mut cmd_buffer: Self::CommandBuffer) {
        cmd_buffer.raw.end_timestamp(&mut cmd_buffer.query);
        let range = cmd_buffer.raw.resolve_timestamp_data(&mut cmd_buffer.query);

        self.timestamp_queries
            .lock()
            .push_back((cmd_buffer.query, Some(range)));
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
    pub(super) query: D::TimestampQuery,
    pub(super) range: Option<Range<usize>>,
    pub(super) frequency: f64,
}

impl<D: RenderDevice> CommandEncoder<D> {
    pub(super) fn new(
        raw: CommandBuffer<D>,
        mapper: Arc<ResourceMapper<D>>,
        query: D::TimestampQuery,
        range: Option<Range<usize>>,
        frequency: f64,
    ) -> Self {
        Self {
            raw,
            mapper,
            query,
            range,
            frequency,
        }
    }
}

pub trait RenderCommandContext<D: RenderDevice> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<D>;
    fn enqueue(&self, cmd: CommandEncoder<D>);
    fn commit(&self, cmd: CommandEncoder<D>);
    fn submit(&self, ty: CommandType) -> SyncPoint;

    fn signal_event(&self, ty: CommandType, event: &D::Event) -> SyncPoint;
    fn wait_event(&self, ty: CommandType, event: &D::Event);
    fn wait_on_cpu(&self, ty: CommandType, value: SyncPoint);
    fn wait_until_complete(&self, ty: CommandType);
    fn wait_idle(&self, ty: CommandType);
}

impl<D: RenderDevice> RenderCommandContext<D> for Context<D> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<D> {
        match ty {
            CommandType::Graphics => self.graphics_queue.create_command_buffer(&self.gpu).cmd(),
            CommandType::Compute => self.compute_queue.create_command_buffer(&self.gpu).cmd(),
            CommandType::Transfer => self.transfer_queue.create_command_buffer(&self.gpu).cmd(),
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

    fn signal_event(&self, ty: CommandType, event: &D::Event) -> SyncPoint {
        match ty {
            CommandType::Graphics => self.graphics_queue.signal_event(event),
            CommandType::Compute => self.compute_queue.signal_event(event),
            CommandType::Transfer => self.transfer_queue.signal_event(event),
        }
    }

    fn wait_event(&self, ty: CommandType, event: &D::Event) {
        match ty {
            CommandType::Graphics => self.graphics_queue.wait_event(event),
            CommandType::Compute => self.compute_queue.wait_event(event),
            CommandType::Transfer => self.transfer_queue.wait_event(event),
        }
    }

    fn wait_on_cpu(&self, ty: CommandType, value: SyncPoint) {
        match ty {
            CommandType::Graphics => self.graphics_queue.wait_on_cpu(value),
            CommandType::Compute => self.compute_queue.wait_on_cpu(value),
            CommandType::Transfer => self.transfer_queue.wait_on_cpu(value),
        }
    }

    fn wait_until_complete(&self, ty: CommandType) {
        match ty {
            CommandType::Graphics => self.graphics_queue.wait_until_complete(),
            CommandType::Compute => self.compute_queue.wait_until_complete(),
            CommandType::Transfer => self.transfer_queue.wait_until_complete(),
        }
    }

    fn wait_idle(&self, ty: CommandType) {
        match ty {
            CommandType::Graphics => self.graphics_queue.wait_idle(),
            CommandType::Compute => self.compute_queue.wait_idle(),
            CommandType::Transfer => self.transfer_queue.wait_idle(),
        }
    }
}

pub trait RenderCommandEncoder<D: RenderDevice> {
    type RenderEncoder<'a>
    where
        Self: 'a;

    fn begin(&mut self, ctx: &Context<D>) -> Option<Timings>;

    fn set_barriers(&mut self, barriers: &[Barrier]);

    fn begin_timestamp(&mut self);

    fn end_timestamp(&mut self);

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

    fn begin(&mut self, ctx: &Context<D>) -> Option<Timings> {
        let timings = self.range.take().map(|range| {
            let ptr: &[u64] = bytemuck::cast_slice(self.query.read_buffer());
            let ptr = &ptr[range];

            let mut prev = ptr[0];
            let timings = ptr[1..]
                .iter()
                .map(|next| {
                    let ms = (*next - prev) as f64 / self.frequency * 1000.0;
                    prev = *next;
                    ms
                })
                .collect::<Vec<_>>();

            Timings { timings }
        });

        self.raw.begin_timestamp(&mut self.query);
        self.raw.begin(&ctx.gpu);

        timings
    }

    fn set_barriers(&mut self, barriers: &[Barrier]) {
        let buffers = self.mapper.buffers.lock();
        let textures = self.mapper.textures.lock();

        let barriers = barriers.iter().map(|b| match b {
            Barrier::Buffer(handle, resource_state) => rhi::command::Barrier::Buffer(
                buffers.get(*handle).expect("failed to get buffer"),
                *resource_state,
            ),
            Barrier::Texture(handle, resource_state) => rhi::command::Barrier::Texture(
                textures.get(*handle).expect("failed to get buffer"),
                *resource_state,
            ),
        });

        self.raw.set_barriers(barriers);
    }

    fn begin_timestamp(&mut self) {
        self.raw.begin_timestamp(&mut self.query);
    }

    fn end_timestamp(&mut self) {
        self.raw.end_timestamp(&mut self.query);
    }

    fn render(
        &mut self,
        targets: &[Handle<Texture>],
        depth: Option<Handle<Texture>>,
    ) -> Self::RenderEncoder<'_> {
        let guard = self.mapper.textures.lock();
        let targets = targets
            .iter()
            .map(|h| guard.get(*h).expect("failed to get texture"));
        let depth = depth.map(|h| guard.get(h).expect("failed to get texture"));

        let raw = self.raw.render(targets, depth);

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
    fn clear_rt(&mut self, texture: Handle<Texture>, color: [f32; 4]);

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
    fn clear_rt(&mut self, texture: Handle<Texture>, color: [f32; 4]) {
        let guard = self.mapper.textures.lock();
        self.raw
            .clear_rt(guard.get(texture).expect("failed to get texture"), color);
    }

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

#[derive(Clone, Copy)]
pub enum Barrier {
    Buffer(Handle<Buffer>, ResourceState),
    Texture(Handle<Texture>, ResourceState),
}
