use std::{
    borrow::Cow,
    collections::VecDeque,
    ops::Range,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use oxidx::dx::{
    self, ICommandAllocator, ICommandQueue, IDevice, IFence, IGraphicsCommandList,
    IGraphicsCommandListExt, IResource, PSO_NONE,
};
use parking_lot::Mutex;
use smallvec::SmallVec;

use crate::rhi::{
    command::{
        Barrier, CommandType, GpuEvent, IoCommandBuffer, RenderCommandBuffer, RenderCommandDevice,
        RenderCommandQueue, RenderEncoder, RenderResourceUploader, Subresource, SyncPoint,
        TransferEncoder,
    },
    resources::{
        Buffer, BufferDesc, BufferUsages, MemoryLocation, QueryHeap, RenderResourceDevice,
    },
    types::{ClearColor, GeomTopology, IndexType, ResourceState, Scissor, Timings, Viewport},
};

use super::{
    conv::{map_command_buffer_type, map_geom_topology, map_resource_state},
    device::DxDevice,
    resources::{DxBuffer, DxTexture, DxTimestampQuery, TextureFlavor},
    shader::{DxRasterPipeline, DxShaderArgument},
};

pub const QUERY_SIZE: usize = 64;

impl RenderCommandDevice for DxDevice {
    type ResourceUploader = DxResourceUploader;
    type CommandQueue = DxCommandQueue;
    type Event = DxFence;

    fn create_command_queue(&self, ty: CommandType, capacity: Option<usize>) -> Self::CommandQueue {
        let queue = self
            .gpu
            .create_command_queue(&dx::CommandQueueDesc::new(map_command_buffer_type(ty)))
            .expect("failed to create command queue");

        let fence = self.create_event(false);

        let frequency = queue
            .get_timestamp_frequency()
            .expect("failed to fetch timestamp frequency") as f64;

        let cmd_allocators = (0..3)
            .map(|_| CommandAllocatorEntry {
                raw: self
                    .gpu
                    .create_command_allocator(map_command_buffer_type(ty))
                    .expect("failed to create command allocator"),
                sync_point: 0,
                query: self.create_timestamp_query(ty, QUERY_SIZE),
                range: None,
                labels: vec![],
            })
            .collect::<VecDeque<_>>();

        let cmd_list = self
            .gpu
            .create_command_list(
                0,
                map_command_buffer_type(ty),
                &cmd_allocators[0].raw,
                PSO_NONE,
            )
            .expect("failed to create command list");
        cmd_list.close().expect("failed to close list");

        DxCommandQueue {
            ty_raw: map_command_buffer_type(ty),
            ty,
            fence,
            capacity,
            cmd_allocators: Mutex::new(cmd_allocators),
            cmd_lists: Mutex::new(vec![cmd_list]),
            in_record: Default::default(),
            pending: Default::default(),
            frequency,

            queue: Mutex::new(queue),
        }
    }

    fn create_resource_uploader(&self) -> Self::ResourceUploader {
        let queue = self.create_command_queue(CommandType::Graphics, None);

        DxResourceUploader {
            queue,
            staging: Default::default(),
            pending: Default::default(),
            res_pool: Default::default(),
        }
    }

    fn create_event(&self, shared: bool) -> Self::Event {
        let flags = if shared {
            dx::FenceFlags::Shared | dx::FenceFlags::SharedCrossAdapter
        } else {
            dx::FenceFlags::empty()
        };

        let fence = self
            .gpu
            .create_fence(0, flags)
            .expect("failed to create fence");

        DxFence {
            fence,
            value: Default::default(),
            shared,
        }
    }

    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event {
        let handle = other_gpu
            .gpu
            .create_shared_handle(&event.fence, None)
            .expect("Failed to open handle");

        let open_fence: dx::Fence = self
            .gpu
            .open_shared_handle(handle)
            .expect("Failed to open heap");
        handle.close().expect("Failed to close handle");

        DxFence {
            fence: open_fence,
            value: Arc::clone(&event.value),
            shared: event.shared,
        }
    }

    fn wait_idle(&self) {}
}

#[derive(Debug)]
pub struct DxCommandQueue {
    ty_raw: dx::CommandListType,
    ty: CommandType,

    fence: DxFence,

    capacity: Option<usize>,
    cmd_allocators: Mutex<VecDeque<CommandAllocatorEntry>>,
    cmd_lists: Mutex<Vec<dx::GraphicsCommandList>>,

    in_record: Mutex<Vec<DxCommandBuffer>>,
    pending: Mutex<Vec<DxCommandBuffer>>,

    frequency: f64,

    pub(crate) queue: Mutex<dx::CommandQueue>,
}

impl RenderCommandQueue for DxCommandQueue {
    type Device = DxDevice;
    type Event = DxFence;
    type CommandBuffer = DxCommandBuffer;

    fn ty(&self) -> CommandType {
        self.ty
    }

    fn frequency(&self) -> f64 {
        self.frequency
    }

    #[allow(unused_assignments)]
    fn create_command_buffer(&self, device: &Self::Device) -> Self::CommandBuffer {
        if let Some(buffer) = self.in_record.lock().pop() {
            return buffer;
        };

        let allocator = if let Some(allocator) =
            self.cmd_allocators.lock().pop_front().and_then(|a| {
                if self.fence.get_completed_value() >= a.sync_point {
                    Some(a)
                } else {
                    None
                }
            }) {
            allocator
                .raw
                .reset()
                .expect("failed to reset command allocator");

            allocator
        } else {
            if self.capacity.is_some() {
                let entry = self.cmd_allocators.lock().pop_front().expect("unreachable");
                self.fence.wait(entry.sync_point);

                entry
            } else {
                CommandAllocatorEntry {
                    raw: device
                        .gpu
                        .create_command_allocator(self.ty_raw)
                        .expect("failed to create command allocator"),
                    sync_point: 0,
                    query: device.create_timestamp_query(self.ty, QUERY_SIZE),
                    range: None,
                    labels: vec![],
                }
            }
        };

        let list = if let Some(list) = self.cmd_lists.lock().pop() {
            list.reset(&allocator.raw, PSO_NONE)
                .expect("Failed to reset list");
            list
        } else {
            let list = device
                .gpu
                .create_command_list(0, self.ty_raw, &allocator.raw, PSO_NONE)
                .expect("failed to create command list");
            list.close().expect("failed to close list");
            list
        };

        DxCommandBuffer {
            list,
            allocator,
            ty: self.ty,
            frequency: self.frequency,
        }
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.in_record.lock().push(cmd_buffer);
    }

    fn commit(&self, mut cmd_buffer: Self::CommandBuffer) {
        cmd_buffer.write_timestamp();
        cmd_buffer.resolve_timestamp_data();

        cmd_buffer.list.close().expect("Failed to close list");
        self.pending.lock().push(cmd_buffer);
    }

    fn submit(&self, _: &DxDevice) -> SyncPoint {
        let cmd_buffers = self.pending.lock().drain(..).collect::<Vec<_>>();
        let lists = cmd_buffers
            .iter()
            .map(|b| Some(b.list.clone()))
            .collect::<SmallVec<[_; 16]>>();

        self.queue.lock().execute_command_lists(&lists);
        let fence_value = self.signal_event(&self.fence);

        let allocators = cmd_buffers.into_iter().map(|mut buffer| {
            buffer.allocator.sync_point = fence_value;
            buffer.allocator
        });
        self.cmd_allocators.lock().extend(allocators);

        let lists = lists
            .into_iter()
            .map(|list| unsafe { list.unwrap_unchecked() });
        self.cmd_lists.lock().extend(lists);

        fence_value
    }

    fn signal_event(&self, event: &Self::Event) -> SyncPoint {
        let value = event.increment();
        self.queue
            .lock()
            .signal(&event.fence, value)
            .expect("Failed to signal");

        value
    }

    fn wait_event(&self, event: &Self::Event) {
        self.queue
            .lock()
            .wait(&event.fence, event.get_goal())
            .expect("failed to wait event");
    }

    fn wait_on_cpu(&self, value: SyncPoint) {
        self.fence.wait(value);
    }

    fn wait_until_complete(&self) {
        self.wait_on_cpu(self.cmd_allocators.lock()[0].sync_point);
    }

    fn wait_idle(&self) {
        let value = self.signal_event(&self.fence);
        self.wait_on_cpu(value);
    }

    fn is_ready(&self) -> bool {
        self.is_ready_for(self.fence.get_goal())
    }

    fn is_ready_for(&self, v: u64) -> bool {
        self.fence.get_completed_value() >= v
    }
}

#[derive(Debug)]
pub struct DxCommandBuffer {
    allocator: CommandAllocatorEntry,
    ty: CommandType,
    frequency: f64,

    list: dx::GraphicsCommandList,
}

impl DxCommandBuffer {
    fn write_timestamp(&mut self) {
        self.list.end_query(
            &self.allocator.query.raw,
            dx::QueryType::Timestamp,
            self.allocator.query.cur_index as u32,
        );
        self.allocator.query.cur_index += 1;
    }
}

impl RenderCommandBuffer for DxCommandBuffer {
    type Device = DxDevice;
    type RenderEncoder<'a> = DxRenderEncoder<'a>;
    type TransferEncoder<'a> = DxTransferEncoder<'a>;

    fn ty(&self) -> CommandType {
        self.ty
    }

    fn begin(&mut self, device: &Self::Device) -> Option<Timings> {
        let timings = self.allocator.range.take().map(|range| {
            let ptr: &[u64] = bytemuck::cast_slice(self.allocator.query.read_buffer());
            let ptr = &ptr[range];

            let timings = if !self.allocator.labels.is_empty() {
                let label_count = self.allocator.labels.len();
                let mut timings_map = Vec::with_capacity(label_count);

                let pass_times = &ptr[1..ptr.len() - 1];

                let durations = pass_times.chunks_exact(2).map(|chunk| {
                    Duration::from_secs_f64((chunk[1] - chunk[0]) as f64 / self.frequency)
                });

                timings_map.extend(
                    durations
                        .zip(self.allocator.labels.drain(..))
                        .map(|(time, label)| (label, time)),
                );
                timings_map
            } else {
                Vec::new()
            };

            let total =
                Duration::from_secs_f64((ptr[ptr.len() - 1] - ptr[0]) as f64 / self.frequency);

            Timings { timings, total }
        });

        self.write_timestamp();
        if self.ty != CommandType::Transfer {
            self.list.set_descriptor_heaps(&[
                Some(device.descriptors.shader_heap.lock().heap.clone()),
                Some(device.descriptors.sampler_heap.lock().heap.clone()),
            ]);
        }

        timings
    }

    fn set_barriers<'a>(&self, barriers: impl IntoIterator<Item = Barrier<'a, DxDevice>>) {
        let barriers = barriers
            .into_iter()
            .filter_map(|b| match b {
                Barrier::Buffer(buffer, resource_state) => {
                    let new_state = map_resource_state(resource_state);
                    let old_state = std::mem::replace(&mut *buffer.state.lock(), new_state);

                    if old_state != new_state {
                        Some(dx::ResourceBarrier::transition(
                            &buffer.raw,
                            old_state,
                            new_state,
                            None,
                        ))
                    } else {
                        None
                    }
                }
                Barrier::Texture(texture, resource_state, sub) => match sub {
                    Subresource::Local(sub) => {
                        let new_state = map_resource_state(resource_state);
                        let old_state = std::mem::replace(&mut *texture.state.lock(), new_state);

                        if old_state != new_state {
                            Some(dx::ResourceBarrier::transition(
                                &texture.raw,
                                old_state,
                                new_state,
                                sub,
                            ))
                        } else {
                            None
                        }
                    }
                    Subresource::Shared => {
                        match &texture.flavor {
                            TextureFlavor::Binded {
                                cross_state, cross, ..
                            } => {
                                let new_state = map_resource_state(resource_state);
                                let old_state =
                                    std::mem::replace(&mut *cross_state.lock(), new_state);

                                if old_state != new_state {
                                    Some(dx::ResourceBarrier::transition(
                                        &cross, old_state, new_state, None,
                                    ))
                                } else {
                                    None
                                }
                            }
                            _ => {
                                None /* NOOP */
                            }
                        }
                    }
                },
            })
            .collect::<SmallVec<[_; 8]>>();

        if !barriers.is_empty() {
            self.list.resource_barrier(&barriers);
        }
    }

    fn render<'a>(
        &mut self,
        label: Cow<'static, str>,
        targets: impl IntoIterator<Item = &'a <Self::Device as RenderResourceDevice>::Texture>,
        depth: Option<&<Self::Device as RenderResourceDevice>::Texture>,
    ) -> Self::RenderEncoder<'_> {
        self.write_timestamp();
        self.allocator.labels.push(label);

        let targets = targets
            .into_iter()
            .filter_map(|t| t.descriptor.as_ref().map(|d| d.cpu))
            .collect::<SmallVec<[_; 8]>>();

        let depth = depth.and_then(|t| t.descriptor.as_ref()).map(|d| d.cpu);

        self.list.om_set_render_targets(&targets, false, depth);

        DxRenderEncoder {
            cmd: self,
            offsets: Default::default(),
        }
    }

    fn resolve_timestamp_data(&mut self) -> std::ops::Range<usize> {
        let range = 0..self.allocator.query.cur_index;

        if self.allocator.query.cur_index > 0 {
            self.list.resolve_query_data(
                &self.allocator.query.raw,
                dx::QueryType::Timestamp,
                0..self.allocator.query.cur_index as u32,
                &self.allocator.query.buffer.raw,
                0,
            );
        }

        self.allocator.range = Some(range.clone());
        self.allocator.query.cur_index = 0;

        range
    }

    fn transfer<'a>(&mut self, label: Cow<'static, str>) -> Self::TransferEncoder<'_> {
        self.write_timestamp();
        self.allocator.labels.push(label);

        DxTransferEncoder { cmd: self }
    }
}

#[derive(Debug)]
struct CommandAllocatorEntry {
    raw: dx::CommandAllocator,
    query: DxTimestampQuery,
    sync_point: SyncPoint,
    range: Option<Range<usize>>,
    labels: Vec<Cow<'static, str>>,
}

pub struct DxResourceUploader {
    queue: DxCommandQueue,
    staging: Mutex<Vec<ResourceEntry>>,

    pending: Mutex<Vec<Vec<DxBuffer>>>,
    res_pool: Mutex<Vec<Vec<DxBuffer>>>,
}

impl RenderCommandQueue for DxResourceUploader {
    type Device = DxDevice;
    type Event = DxFence;
    type CommandBuffer = DxIoCommandBuffer;

    fn ty(&self) -> CommandType {
        CommandType::Transfer
    }

    fn frequency(&self) -> f64 {
        self.queue.frequency
    }

    fn create_command_buffer(&self, device: &Self::Device) -> Self::CommandBuffer {
        let buffer = self.queue.create_command_buffer(device);
        let temps = self.res_pool.lock().pop().unwrap_or_default();

        DxIoCommandBuffer { buffer, temps }
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.res_pool.lock().push(cmd_buffer.temps);
        self.queue.enqueue(cmd_buffer.buffer);
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        self.pending.lock().push(cmd_buffer.temps);
        self.queue.commit(cmd_buffer.buffer);
    }

    fn submit(&self, device: &DxDevice) -> SyncPoint {
        let value = self.queue.submit(device);

        {
            let mut guard = self.pending.lock();
            let pendings = guard.drain(..).flatten().map(|res| ResourceEntry {
                res,
                sync_point: value,
            });

            self.staging.lock().extend(pendings);
        }

        let completed = self.queue.fence.get_completed_value();

        {
            let mut guard = self.staging.lock();

            let idx = guard
                .iter()
                .take_while(|res| res.sync_point <= completed)
                .count();

            if idx > 0 {
                let drained = guard.drain(0..idx);

                for buffer in drained {
                    device.destroy_buffer(buffer.res);
                }
            }
        }

        value
    }

    fn signal_event(&self, event: &Self::Event) -> SyncPoint {
        self.queue.signal_event(event)
    }

    fn wait_event(&self, event: &Self::Event) {
        self.queue.wait_event(event);
    }

    fn wait_on_cpu(&self, value: SyncPoint) {
        self.queue.wait_on_cpu(value);
    }

    fn wait_until_complete(&self) {
        self.queue.wait_until_complete();
    }

    fn wait_idle(&self) {
        self.queue.wait_idle();
    }

    fn is_ready(&self) -> bool {
        self.queue.is_ready()
    }

    fn is_ready_for(&self, v: u64) -> bool {
        self.queue.is_ready_for(v)
    }
}

impl RenderResourceUploader for DxResourceUploader {
    fn flush(&self, device: &Self::Device) {
        self.wait_idle();

        for buffer in self.staging.lock().drain(..) {
            device.destroy_buffer(buffer.res);
        }
    }
}

#[derive(Debug)]
pub struct DxIoCommandBuffer {
    buffer: DxCommandBuffer,
    temps: Vec<DxBuffer>,
}

impl IoCommandBuffer for DxIoCommandBuffer {
    type Device = DxDevice;

    fn load_to_buffer(&mut self, device: &Self::Device, buffer: &mut DxBuffer, data: &'_ [u8]) {
        if buffer.desc.memory_location == MemoryLocation::CpuToGpu {
            let map = buffer.map_mut();
            map.clone_from_slice(data);
        } else {
            let mut staging =
                device.create_buffer(BufferDesc::cpu_to_gpu(buffer.desc.size, BufferUsages::Copy));

            {
                let map = staging.map_mut();
                map.clone_from_slice(data);
            }

            self.buffer.list.copy_resource(&buffer.raw, &staging.raw);

            self.temps.push(staging);
        }
    }

    fn load_to_texture(&mut self, device: &Self::Device, texture: &DxTexture, data: &'_ [u8]) {
        debug_assert_eq!(data.len(), texture.size);

        let staging =
            device.create_buffer(BufferDesc::cpu_to_gpu(texture.size, BufferUsages::Copy));

        self.buffer.set_barriers(
            [Barrier::Texture(
                texture,
                ResourceState::CopyDst,
                Subresource::Local(None),
            )]
            .into_iter(),
        );

        let copied = self.buffer.list.update_subresources_fixed::<1, _, _>(
            &texture.raw,
            &staging.raw,
            0,
            0..1,
            &[dx::SubresourceData::new(data).with_row_pitch(
                (texture.desc.format.bytes_per_pixel() * texture.desc.extent[0] as usize) as isize,
            )],
        );

        debug_assert!(copied > 0);

        self.buffer.set_barriers(
            [Barrier::Texture(
                texture,
                ResourceState::Shader,
                Subresource::Local(None),
            )]
            .into_iter(),
        );

        self.temps.push(staging);
    }
}

#[derive(Debug)]
struct ResourceEntry {
    res: DxBuffer,
    sync_point: SyncPoint,
}

#[derive(Debug)]
pub struct DxFence {
    pub(super) fence: dx::Fence,
    pub(super) value: Arc<AtomicU64>,
    pub(super) shared: bool,
}

impl GpuEvent for DxFence {
    fn wait(&self, value: SyncPoint) -> bool {
        if self.get_completed_value() < value {
            let event = dx::Event::create(false, false).expect("failed to create event");
            self.fence
                .set_event_on_completion(value, event)
                .expect("failed to bind fence to event");
            if event.wait(10_000_000) == 0x00000102 {
                panic!("device lost")
            }

            true
        } else {
            false
        }
    }

    fn increment(&self) -> SyncPoint {
        self.value
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1
    }

    fn get_completed_value(&self) -> SyncPoint {
        self.fence.get_completed_value()
    }

    fn get_goal(&self) -> SyncPoint {
        self.value.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct DxRenderEncoder<'a> {
    pub(super) cmd: &'a mut DxCommandBuffer,
    pub(super) offsets: [usize; 4],
}

impl<'a> RenderEncoder for DxRenderEncoder<'a> {
    type Buffer = DxBuffer;
    type Texture = DxTexture;
    type RasterPipeline = DxRasterPipeline;
    type ShaderArgument = DxShaderArgument;

    fn clear_rt(&self, texture: &Self::Texture, color: Option<[f32; 4]>) {
        if let Some(descriptor) = &texture.descriptor {
            let color = match (texture.desc.clear_color, color) {
                (Some(ClearColor::Color(c)), None) => c,
                (Some(ClearColor::Color(_)), Some(c)) => c,
                (None, Some(c)) => c,
                (_, _) => Default::default(),
            };

            self.cmd
                .list
                .clear_render_target_view(descriptor.cpu, color, &[]);
        }
    }

    fn clear_depth(&self, texture: &Self::Texture, depth: Option<f32>) {
        if let Some(descriptor) = &texture.descriptor {
            let depth = match (texture.desc.clear_color, depth) {
                (Some(ClearColor::Depth(c)), None) => c,
                (Some(ClearColor::Depth(_)), Some(c)) => c,
                (None, Some(c)) => c,
                (_, _) => Default::default(),
            };

            self.cmd.list.clear_depth_stencil_view(
                descriptor.cpu,
                dx::ClearFlags::Depth,
                depth,
                0,
                None,
            );
        }
    }

    fn set_viewport(&self, viewport: Viewport) {
        self.cmd
            .list
            .rs_set_viewports(&[dx::Viewport::from_position_and_size(
                (viewport.x, viewport.y),
                (viewport.w, viewport.h),
            )]);
    }

    fn set_scissor(&self, scissor: Scissor) {
        self.cmd.list.rs_set_scissor_rects(&[dx::Rect::default()
            .with_left(scissor.x)
            .with_top(scissor.y)
            .with_size((scissor.w as i32, scissor.h as i32))]);
    }

    fn set_topology(&self, topology: GeomTopology) {
        self.cmd
            .list
            .ia_set_primitive_topology(map_geom_topology(topology));
    }

    fn set_raster_pipeline(&mut self, pipeline: &Self::RasterPipeline) {
        self.cmd.list.set_pipeline_state(&pipeline.raw);

        if let Some(layout) = &pipeline.layout {
            self.cmd.list.set_graphics_root_signature(Some(&layout.raw));
            self.offsets = layout.offsets;
        }
    }

    fn bind_shader_argument(
        &self,
        space: u32,
        argument: &Self::ShaderArgument,
        dynamic_offset: usize,
    ) {
        let base = self.offsets[space as usize] as u32;
        let mut offset = 0;
        if let Some(address) = &argument.dynamic_address {
            assert_eq!((address + dynamic_offset as u64) % 256, 0);
            self.cmd
                .list
                .set_graphics_root_constant_buffer_view(base, *address + dynamic_offset as u64);
            offset += 1;
        }

        if let Some(d) = &argument.views {
            self.cmd
                .list
                .set_graphics_root_descriptor_table(base + offset, d.gpu);
            offset += 1;
        }

        if let Some(d) = &argument.samplers {
            self.cmd
                .list
                .set_graphics_root_descriptor_table(base + offset, d.gpu);
        }
    }

    fn bind_vertex_buffer(&self, buffer: &Self::Buffer, slot: usize) {
        self.cmd.list.ia_set_vertex_buffers(
            slot as u32,
            &[dx::VertexBufferView::new(
                buffer.raw.get_gpu_virtual_address(),
                buffer.desc.stride as u32,
                buffer.desc.size as u32,
            )],
        );
    }

    fn bind_index_buffer(&self, buffer: &Self::Buffer, ty: IndexType) {
        self.cmd
            .list
            .ia_set_index_buffer(Some(&dx::IndexBufferView::new(
                buffer.raw.get_gpu_virtual_address(),
                buffer.desc.size as u32,
                match ty {
                    IndexType::U16 => dx::Format::R16Uint,
                    IndexType::U32 => dx::Format::R32Uint,
                },
            )));
    }

    fn draw(&self, count: u32, start_vertex: u32) {
        self.cmd.list.draw_instanced(count, 1, start_vertex, 0);
    }

    fn draw_indexed(&self, count: u32, start_index: u32, base_vertex: u32) {
        self.cmd
            .list
            .draw_indexed_instanced(count, 1, start_index, base_vertex as i32, 0);
    }
}

impl Drop for DxRenderEncoder<'_> {
    fn drop(&mut self) {
        self.cmd.write_timestamp();
    }
}

#[derive(Debug)]
pub struct DxTransferEncoder<'a> {
    pub(super) cmd: &'a mut DxCommandBuffer,
}

impl<'a> TransferEncoder for DxTransferEncoder<'a> {
    type Texture = DxTexture;

    fn pull_texture(&self, texture: &Self::Texture) {
        match &texture.flavor {
            TextureFlavor::Binded { cross, .. } => {
                self.cmd.list.copy_resource(&texture.raw, cross);
            }
            _ => { /* NOOP */ }
        }
    }

    fn push_texture(&self, texture: &Self::Texture) {
        match &texture.flavor {
            TextureFlavor::Binded { cross, .. } => {
                self.cmd.list.copy_resource(cross, &texture.raw);
            }
            _ => { /* NOOP */ }
        }
    }
}

impl Drop for DxTransferEncoder<'_> {
    fn drop(&mut self) {
        self.cmd.write_timestamp();
    }
}
