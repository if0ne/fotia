use std::{
    collections::VecDeque,
    sync::{Arc, atomic::AtomicU64},
};

use oxidx::dx::{
    self, ICommandAllocator, ICommandQueue, IDevice, IFence, IGraphicsCommandList,
    IGraphicsCommandListExt, PSO_NONE,
};
use parking_lot::Mutex;

use crate::rhi::{
    command::{
        CommandType, GpuEvent, IoCommandBuffer, RenderCommandDevice, RenderCommandQueue,
        RenderResourceUploader, SyncPoint,
    },
    resources::{BufferDesc, BufferUsages, MemoryLocation, RenderResourceDevice},
};

use super::{
    conv::map_command_buffer_type,
    device::DxDevice,
    resources::{DxBuffer, DxTexture},
};

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

        let frequency = 1000.0
            / queue
                .get_timestamp_frequency()
                .expect("failed to fetch timestamp frequency") as f64;

        let cmd_allocators = (0..3)
            .map(|_| CommandAllocatorEntry {
                raw: self
                    .gpu
                    .create_command_allocator(map_command_buffer_type(ty))
                    .expect("failed to create command allocator"),
                sync_point: 0,
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
            device: self.gpu.clone(),
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
        let queue = self.create_command_queue(CommandType::Transfer, None);

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
    device: dx::Device,
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

impl RenderCommandQueue<DxDevice> for DxCommandQueue {
    type Event = DxFence;
    type CommandBuffer = DxCommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer {
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
                    raw: self
                        .device
                        .create_command_allocator(self.ty_raw)
                        .expect("failed to create command allocator"),
                    sync_point: 0,
                }
            }
        };

        let list = if let Some(list) = self.cmd_lists.lock().pop() {
            list.reset(&allocator.raw, PSO_NONE)
                .expect("Failed to reset list");
            list
        } else {
            let list = self
                .device
                .create_command_list(0, self.ty_raw, &allocator.raw, PSO_NONE)
                .expect("failed to create command list");
            list.close().expect("failed to close list");
            list
        };

        DxCommandBuffer { list, allocator }
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        self.in_record.lock().push(cmd_buffer);
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        cmd_buffer.list.close().expect("Failed to close list");
        self.pending.lock().push(cmd_buffer);
    }

    fn submit(&self, _: &DxDevice) -> SyncPoint {
        let cmd_buffers = self.pending.lock().drain(..).collect::<Vec<_>>();
        let lists = cmd_buffers
            .iter()
            .map(|b| Some(b.list.clone()))
            .collect::<Vec<_>>();

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
}

#[derive(Debug)]
pub struct DxCommandBuffer {
    allocator: CommandAllocatorEntry,
    pub(super) list: dx::GraphicsCommandList,
}

#[derive(Debug)]
struct CommandAllocatorEntry {
    raw: dx::CommandAllocator,
    sync_point: SyncPoint,
}

pub struct DxResourceUploader {
    queue: DxCommandQueue,
    staging: Mutex<Vec<ResourceEntry>>,

    pending: Mutex<Vec<Vec<DxBuffer>>>,
    res_pool: Mutex<Vec<Vec<DxBuffer>>>,
}

impl RenderCommandQueue<DxDevice> for DxResourceUploader {
    type Event = DxFence;
    type CommandBuffer = DxIoCommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer {
        let buffer = self.queue.create_command_buffer();
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
}

impl RenderResourceUploader<DxDevice> for DxResourceUploader {}

#[derive(Debug)]
pub struct DxIoCommandBuffer {
    buffer: DxCommandBuffer,
    temps: Vec<DxBuffer>,
}

impl IoCommandBuffer for DxIoCommandBuffer {
    type Device = DxDevice;

    fn load_to_buffer(&mut self, device: &Self::Device, buffer: &DxBuffer, data: &'_ [u8]) {
        if buffer.desc.memory_location == MemoryLocation::CpuToGpu || device.desc.is_uma {
            let map = buffer.map();
            map.pointer.clone_from_slice(data);
        } else {
            let staging =
                device.create_buffer(BufferDesc::cpu_to_gpu(buffer.desc.size, BufferUsages::Copy));

            {
                let map = staging.map();
                map.pointer.clone_from_slice(data);
            }

            self.buffer.list.copy_resource(&buffer.raw, &staging.raw);

            self.temps.push(staging);
        }
    }

    fn load_to_texture(&mut self, device: &Self::Device, texture: &DxTexture, data: &'_ [u8]) {
        let staging =
            device.create_buffer(BufferDesc::cpu_to_gpu(texture.size, BufferUsages::Copy));

        self.buffer.list.update_subresources_fixed::<1, _, _>(
            &texture.raw,
            &staging.raw,
            0,
            0..1,
            &[dx::SubresourceData::new(data).with_row_pitch(
                texture.desc.format.bytes_per_pixel() * texture.desc.extent[0] as usize,
            )],
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
