use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, VecDeque},
    ops::Range,
    sync::Arc,
    time::Duration,
};

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

pub(super) struct TimestampEntry<D: RenderDevice> {
    query: D::TimestampQuery,
    range: Option<Range<usize>>,
    labels: Vec<Cow<'static, str>>,
    is_used: bool,
}

impl<D: RenderDevice> TimestampEntry<D> {
    pub fn new(query: D::TimestampQuery) -> Self {
        Self {
            query,
            range: None,
            labels: vec![],
            is_used: false,
        }
    }
}

pub struct CommandQueue<D: RenderDevice> {
    pub(super) raw: D::CommandQueue,
    pub(super) mapper: Arc<ResourceMapper<D>>,
    pub(super) timestamp_queries: Mutex<VecDeque<TimestampEntry<D>>>,
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
                timestamp_queries
                    .into_iter()
                    .map(|q| TimestampEntry::new(q))
                    .collect(),
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
                let mut guard = self.timestamp_queries.lock();
                let TimestampEntry {
                    query,
                    range,
                    labels,
                    ..
                } = if guard.front().is_some_and(|q| !q.is_used) {
                    guard.pop_front().unwrap_or_else(|| {
                        TimestampEntry::new(device.create_timestamp_query(self.ty(), QUERY_SIZE))
                    })
                } else {
                    TimestampEntry::new(device.create_timestamp_query(self.ty(), QUERY_SIZE))
                };

                CommandBufferState::New(Self::CommandBuffer::new(
                    cmd,
                    Arc::clone(&self.mapper),
                    query,
                    range,
                    labels,
                    self.frequency(),
                ))
            }
            CommandBufferState::Stashed(cmd) => {
                let TimestampEntry {
                    query,
                    range,
                    labels,
                    ..
                } = self
                    .timestamp_queries
                    .lock()
                    .pop_front()
                    .expect("wrong count of timestamp heaps");

                CommandBufferState::Stashed(Self::CommandBuffer::new(
                    cmd,
                    Arc::clone(&self.mapper),
                    query,
                    range,
                    labels,
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
                    vec![],
                    self.frequency(),
                ))
            }
        }
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.timestamp_queries.lock().push_front(TimestampEntry {
            query: cmd_buffer.query.into_inner(),
            range: cmd_buffer.range,
            labels: cmd_buffer.labels,
            is_used: true,
        });
        self.raw.enqueue(cmd_buffer.raw);
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        cmd_buffer.write_timestamp();
        let range = cmd_buffer
            .raw
            .resolve_timestamp_data(&mut *cmd_buffer.query.borrow_mut());

        self.timestamp_queries.lock().push_back(TimestampEntry {
            query: cmd_buffer.query.into_inner(),
            range: Some(range),
            labels: cmd_buffer.labels,
            is_used: true,
        });
        self.raw.commit(cmd_buffer.raw);
    }

    fn submit(&self, device: &Self::Device) -> SyncPoint {
        self.timestamp_queries
            .lock()
            .iter_mut()
            .for_each(|q| q.is_used = false);
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
    pub(super) query: RefCell<D::TimestampQuery>,
    pub(super) range: Option<Range<usize>>,
    pub(super) labels: Vec<Cow<'static, str>>,
    pub(super) frequency: f64,
}

impl<D: RenderDevice> CommandEncoder<D> {
    pub(super) fn new(
        raw: CommandBuffer<D>,
        mapper: Arc<ResourceMapper<D>>,
        query: D::TimestampQuery,
        range: Option<Range<usize>>,
        labels: Vec<Cow<'static, str>>,
        frequency: f64,
    ) -> Self {
        Self {
            raw,
            mapper,
            query: RefCell::new(query),
            range,
            labels,
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

    fn render(
        &mut self,
        label: Cow<'static, str>,
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
            let query = self.query.borrow();
            let ptr: &[u64] = bytemuck::cast_slice(query.read_buffer());
            let ptr = &ptr[range];

            let timings = if self.labels.len() > 0 {
                let mut prev = ptr[1];
                let timings = ptr[2..]
                    .iter()
                    .map(|next| {
                        let ms = Duration::from_secs_f64((*next - prev) as f64 / self.frequency);
                        prev = *next;
                        ms
                    })
                    .zip(self.labels.drain(..))
                    .map(|(time, label)| (label, time))
                    .collect::<HashMap<_, _>>();

                timings
            } else {
                HashMap::new()
            };

            let total =
                Duration::from_secs_f64((ptr[ptr.len() - 1] - ptr[0]) as f64 / self.frequency);

            Timings { timings, total }
        });

        self.write_timestamp();
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

    fn render(
        &mut self,
        label: Cow<'static, str>,
        targets: &[Handle<Texture>],
        depth: Option<Handle<Texture>>,
    ) -> Self::RenderEncoder<'_> {
        self.write_timestamp();
        self.labels.push(label);

        let guard = self.mapper.textures.lock();
        let targets = targets
            .iter()
            .map(|h| guard.get(*h).expect("failed to get texture"));
        let depth = depth.map(|h| guard.get(h).expect("failed to get texture"));

        let raw = self.raw.render(targets, depth);

        Self::RenderEncoder {
            cmd: self,
            raw,
            mapper: &self.mapper,
        }
    }
}

impl<D: RenderDevice> CommandEncoder<D> {
    pub(super) fn write_timestamp(&self) {
        self.raw.write_timestamp(&mut *self.query.borrow_mut());
    }
}

pub struct RenderEncoderImpl<'a, D: RenderDevice> {
    pub(super) cmd: &'a CommandEncoder<D>,
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

impl<D: RenderDevice> Drop for RenderEncoderImpl<'_, D> {
    fn drop(&mut self) {
        self.cmd.write_timestamp();
    }
}

#[derive(Clone, Copy)]
pub enum Barrier {
    Buffer(Handle<Buffer>, ResourceState),
    Texture(Handle<Texture>, ResourceState),
}
