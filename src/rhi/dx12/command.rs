use std::{
    collections::VecDeque,
    sync::{Arc, atomic::AtomicU64},
};

use oxidx::dx::{
    self, ICommandAllocator, ICommandQueue, IDevice, IFence, IGraphicsCommandList, PSO_NONE,
};
use parking_lot::Mutex;

use crate::rhi::command::{
    CommandType, GpuEvent, IoCommandBuffer, RenderCommandDevice, RenderCommandQueue,
    RenderResourceUploader, SyncPoint,
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
        DxResourceUploader { queue }
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

impl RenderCommandQueue for DxCommandQueue {
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

    fn submit(&self) -> SyncPoint {
        {
            let mut guard = self.in_record.lock();
            let in_record = guard.drain(..);
            self.pending.lock().extend(in_record);
        }

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
}

impl RenderCommandQueue for DxResourceUploader {
    type Event = ();
    type CommandBuffer = DxIoCommandBuffer;

    fn create_command_buffer(&self) -> Self::CommandBuffer {
        todo!()
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        todo!()
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        todo!()
    }

    fn submit(&self) -> SyncPoint {
        todo!()
    }

    fn signal_event(&self, event: &Self::Event) -> SyncPoint {
        todo!()
    }

    fn wait_event(&self, event: &Self::Event) {
        todo!()
    }

    fn wait_on_cpu(&self, value: SyncPoint) {
        todo!()
    }

    fn wait_until_complete(&self) {
        todo!()
    }

    fn wait_idle(&self) {
        todo!()
    }
}

impl RenderResourceUploader<DxDevice> for DxResourceUploader {}

#[derive(Debug)]
pub struct DxIoCommandBuffer {
    buffer: DxCommandBuffer,
}

impl IoCommandBuffer<DxDevice> for DxIoCommandBuffer {
    fn load_to_buffer(&self, buffer: &DxBuffer, data: &'_ [u8]) {
        todo!()
    }

    fn load_to_texture(&self, texture: &DxTexture, data: &'_ [u8]) {
        todo!()
    }
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
