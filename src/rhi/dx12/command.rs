use std::sync::{Arc, atomic::AtomicU64};

use oxidx::dx::{self, IDevice, IFence};

use crate::rhi::command::{
    GpuEvent, IoCommandBuffer, RenderCommandDevice, RenderCommandQueue, RenderResourceUploader,
    SyncPoint,
};

use super::{
    device::DxDevice,
    resources::{DxBuffer, DxTexture},
};

impl RenderCommandDevice for DxDevice {
    type ResourceUploader = DxResourceUploader;
    type CommandQueue = DxCommandQueue;
    type Event = DxFence;

    fn create_command_queue(
        &self,
        ty: crate::rhi::command::CommandType,
        capacity: Option<usize>,
    ) -> Self::CommandQueue {
        DxCommandQueue {}
    }

    fn create_resource_uploader(&self) -> Self::ResourceUploader {
        DxResourceUploader {}
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

    fn wait_idle(&self) {
        todo!()
    }
}

pub struct DxCommandQueue {}

impl RenderCommandQueue for DxCommandQueue {
    type Event = DxFence;
    type CommandBuffer = ();

    fn create_command_buffer(&self) -> Self::CommandBuffer {
        todo!()
    }

    fn enqueue(&self, cmd_buffer: Self::CommandBuffer) {
        todo!()
    }

    fn commit(&self, cmd_buffer: Self::CommandBuffer) {
        todo!()
    }

    fn submit(&self) -> crate::rhi::command::SyncPoint {
        todo!()
    }

    fn signal_event(&self, event: Self::Event, value: crate::rhi::command::SyncPoint) {
        todo!()
    }

    fn wait_event(&self, event: Self::Event, value: crate::rhi::command::SyncPoint) {
        todo!()
    }

    fn wait_on_cpu(&self, value: crate::rhi::command::SyncPoint) {
        todo!()
    }

    fn wait_until_complete(&self) {
        todo!()
    }

    fn wait_idle(&self) {
        todo!()
    }
}

pub struct DxResourceUploader {}

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

    fn submit(&self) -> crate::rhi::command::SyncPoint {
        todo!()
    }

    fn signal_event(&self, event: Self::Event, value: crate::rhi::command::SyncPoint) {
        todo!()
    }

    fn wait_event(&self, event: Self::Event, value: crate::rhi::command::SyncPoint) {
        todo!()
    }

    fn wait_on_cpu(&self, value: crate::rhi::command::SyncPoint) {
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

pub struct DxIoCommandBuffer {}

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
