use crate::rhi::command::{
    IoCommandBuffer, RenderCommandDevice, RenderCommandQueue, RenderResourceUploader,
};

use super::{
    device::DxDevice,
    resources::{DxBuffer, DxTexture},
};

impl RenderCommandDevice for DxDevice {
    type ResourceUploader = DxResourceUploader;
    type CommandQueue = DxCommandQueue;
    type Event = ();

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
        todo!()
    }

    fn open_event(&self, event: &Self::Event, other_gpu: &Self) -> Self::Event {
        todo!()
    }

    fn wait_idle(&self) {
        todo!()
    }
}

pub struct DxCommandQueue {}

impl RenderCommandQueue for DxCommandQueue {
    type Event = ();
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
