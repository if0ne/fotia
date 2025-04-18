use std::{borrow::Cow, sync::Arc};

use crate::{
    collections::handle::Handle,
    rhi::{
        self,
        command::{
            CommandType, RenderCommandBuffer, RenderCommandDevice, RenderCommandQueue,
            RenderEncoder as _, Subresource, SyncPoint, TransferEncoder as _,
        },
        types::{GeomTopology, IndexType, ResourceState, Scissor, Timings, Viewport},
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
type TransferEncoderType<'a, D> = <CommandBuffer<D> as RenderCommandBuffer>::TransferEncoder<'a>;

#[derive(Clone, Copy)]
pub enum Barrier {
    Buffer(Handle<Buffer>, ResourceState),
    Texture(Handle<Texture>, ResourceState, Subresource),
}

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

    fn ty(&self) -> CommandType {
        self.raw.ty()
    }

    fn frequency(&self) -> f64 {
        self.raw.frequency()
    }

    fn create_command_buffer(&self, device: &Self::Device) -> Self::CommandBuffer {
        let cmd = self.raw.create_command_buffer(&device);

        Self::CommandBuffer::new(cmd, Arc::clone(&self.mapper))
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

    fn is_ready(&self) -> bool {
        self.raw.is_ready()
    }

    fn is_ready_for(&self, v: u64) -> bool {
        self.raw.is_ready_for(v)
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

    fn signal_event(&self, ty: CommandType, event: &D::Event) -> SyncPoint;
    fn wait_event(&self, ty: CommandType, event: &D::Event);
    fn wait_on_cpu(&self, ty: CommandType, value: SyncPoint);
    fn wait_until_complete(&self, ty: CommandType);
    fn wait_idle(&self, ty: CommandType);
    fn is_ready(&self, ty: CommandType) -> bool;
    fn is_ready_for(&self, ty: CommandType, v: SyncPoint) -> bool;
}

impl<D: RenderDevice> RenderCommandContext<D> for Context<D> {
    fn create_encoder(&self, ty: CommandType) -> CommandEncoder<D> {
        match ty {
            CommandType::Graphics => self.graphics_queue.create_command_buffer(&self.gpu),
            CommandType::Compute => self.compute_queue.create_command_buffer(&self.gpu),
            CommandType::Transfer => self.transfer_queue.create_command_buffer(&self.gpu),
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

    fn is_ready(&self, ty: CommandType) -> bool {
        match ty {
            CommandType::Graphics => self.graphics_queue.is_ready(),
            CommandType::Compute => self.compute_queue.is_ready(),
            CommandType::Transfer => self.transfer_queue.is_ready(),
        }
    }

    fn is_ready_for(&self, ty: CommandType, v: SyncPoint) -> bool {
        match ty {
            CommandType::Graphics => self.graphics_queue.is_ready_for(v),
            CommandType::Compute => self.compute_queue.is_ready_for(v),
            CommandType::Transfer => self.transfer_queue.is_ready_for(v),
        }
    }
}

pub trait RenderCommandEncoder<D: RenderDevice> {
    type RenderEncoder<'a>
    where
        Self: 'a;

    type TransferEncoder<'a>
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

    fn transfer(&mut self, label: Cow<'static, str>) -> Self::TransferEncoder<'_>;
}

impl<D: RenderDevice> RenderCommandEncoder<D> for CommandEncoder<D> {
    type RenderEncoder<'a>
        = RenderEncoderImpl<'a, D>
    where
        D: 'a;

    type TransferEncoder<'a>
        = TransferEncoderImpl<'a, D>
    where
        D: 'a;

    fn begin(&mut self, ctx: &Context<D>) -> Option<Timings> {
        self.raw.begin(&ctx.gpu)
    }

    fn set_barriers(&mut self, barriers: &[Barrier]) {
        let buffers = self.mapper.buffers.read();
        let textures = self.mapper.textures.read();

        let barriers = barriers.iter().map(|b| match b {
            Barrier::Buffer(handle, resource_state) => rhi::command::Barrier::Buffer(
                buffers.get(*handle).expect("failed to get buffer"),
                *resource_state,
            ),
            Barrier::Texture(handle, resource_state, sub) => rhi::command::Barrier::Texture(
                textures.get(*handle).expect("failed to get buffer"),
                *resource_state,
                *sub,
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
        let guard = self.mapper.textures.read();
        let targets = targets
            .iter()
            .map(|h| guard.get(*h).expect("failed to get texture"));
        let depth = depth.map(|h| guard.get(h).expect("failed to get texture"));

        let raw = self.raw.render(label, targets, depth);

        Self::RenderEncoder {
            raw,
            mapper: &self.mapper,
            active_vbs: Default::default(),
            active_ibs: Default::default(),
            active_arguments: Default::default(),
            active_dyn_offsets: Default::default(),
        }
    }

    fn transfer(&mut self, label: Cow<'static, str>) -> Self::TransferEncoder<'_> {
        let raw = self.raw.transfer(label);

        Self::TransferEncoder {
            raw,
            mapper: &self.mapper,
        }
    }
}

pub struct RenderEncoderImpl<'a, D: RenderDevice> {
    pub(super) raw: RenderEncoderType<'a, D>,
    pub(super) mapper: &'a ResourceMapper<D>,
    active_vbs: [Option<Handle<Buffer>>; 16],
    active_arguments: [Option<Handle<ShaderArgument>>; 4],
    active_dyn_offsets: [usize; 4],
    active_ibs: Option<Handle<Buffer>>,
}

pub trait RenderEncoder {
    fn clear_rt(&mut self, texture: Handle<Texture>, color: Option<[f32; 4]>);
    fn clear_depth(&mut self, texture: Handle<Texture>, depth: Option<f32>);

    fn set_viewport(&mut self, viewport: Viewport);
    fn set_scissor(&mut self, scissor: Scissor);
    fn set_topology(&self, topology: GeomTopology);

    fn set_render_pipeline(&mut self, pipeline: Handle<RasterPipeline>);
    fn bind_shader_argument(
        &mut self,
        space: u32,
        argument: Handle<ShaderArgument>,
        dynamic_offset: usize,
    );

    fn bind_vertex_buffer(&mut self, buffer: Handle<Buffer>, slot: usize);
    fn bind_index_buffer(&mut self, buffer: Handle<Buffer>, ty: IndexType);

    fn draw(&mut self, count: u32, start_vertex: u32);
    fn draw_indexed(&mut self, count: u32, start_index: u32, base_index: u32);
}

impl<'a, D: RenderDevice> RenderEncoder for RenderEncoderImpl<'a, D> {
    fn clear_rt(&mut self, texture: Handle<Texture>, color: Option<[f32; 4]>) {
        let guard = self.mapper.textures.read();
        self.raw
            .clear_rt(guard.get(texture).expect("failed to get texture"), color);
    }

    fn clear_depth(&mut self, texture: Handle<Texture>, depth: Option<f32>) {
        let guard = self.mapper.textures.read();
        self.raw
            .clear_depth(guard.get(texture).expect("failed to get texture"), depth);
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        self.raw.set_viewport(viewport);
    }

    fn set_scissor(&mut self, scissor: Scissor) {
        self.raw.set_scissor(scissor);
    }

    fn set_topology(&self, topology: GeomTopology) {
        self.raw.set_topology(topology);
    }

    fn set_render_pipeline(&mut self, pipeline: Handle<RasterPipeline>) {
        let guard = self.mapper.raster_pipelines.read();
        let pipeline = guard.get(pipeline).expect("failed to get pipeline");

        self.raw.set_raster_pipeline(pipeline);
    }

    fn bind_shader_argument(
        &mut self,
        set: u32,
        argument: Handle<ShaderArgument>,
        dynamic_offset: usize,
    ) {
        if self.active_arguments[set as usize] == Some(argument)
            && self.active_dyn_offsets[set as usize] == dynamic_offset
        {
            return;
        } else {
            self.active_arguments[set as usize] = Some(argument);
            self.active_dyn_offsets[set as usize] = dynamic_offset;
        }

        let guard = self.mapper.shader_arguments.read();
        let argument = guard.get(argument).expect("failed to get shader argument");

        self.raw.bind_shader_argument(set, argument, dynamic_offset);
    }

    fn bind_vertex_buffer(&mut self, buffer: Handle<Buffer>, slot: usize) {
        if self.active_vbs[slot] == Some(buffer) {
            return;
        } else {
            self.active_vbs[slot] = Some(buffer);
        }

        let guard = self.mapper.buffers.read();
        let buffer = guard.get(buffer).expect("failed to get buffer");

        self.raw.bind_vertex_buffer(buffer, slot);
    }

    fn bind_index_buffer(&mut self, buffer: Handle<Buffer>, ty: IndexType) {
        if self.active_ibs == Some(buffer) {
            return;
        } else {
            self.active_ibs = Some(buffer);
        }

        let guard = self.mapper.buffers.read();
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

pub struct TransferEncoderImpl<'a, D: RenderDevice> {
    pub(super) raw: TransferEncoderType<'a, D>,
    pub(super) mapper: &'a ResourceMapper<D>,
}

pub trait TransferEncoder {
    fn pull_texture(&self, texture: Handle<Texture>);
    fn push_texture(&self, texture: Handle<Texture>);
}

impl<'a, D: RenderDevice> TransferEncoder for TransferEncoderImpl<'a, D> {
    fn pull_texture(&self, texture: Handle<Texture>) {
        let guard = self.mapper.textures.read();

        self.raw
            .pull_texture(guard.get(texture).expect("failed to get texture"));
    }

    fn push_texture(&self, texture: Handle<Texture>) {
        let guard = self.mapper.textures.read();

        self.raw
            .push_texture(guard.get(texture).expect("failed to get texture"));
    }
}
