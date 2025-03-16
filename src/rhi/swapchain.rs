use winit::raw_window_handle::RawWindowHandle;

use super::command::SyncPoint;

#[derive(Clone, Debug)]
pub enum PresentMode {
    Immediate,
    Mailbox,
    Fifo,
}

#[derive(Clone, Debug)]
pub struct SwapchainDesc {
    pub width: u32,
    pub height: u32,
    pub present_mode: PresentMode,
    pub frames: usize,
}

pub trait RenderSwapchainDevice {
    type Swapchain: Surface;
    type Queue;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &RawWindowHandle,
        queue: &Self::Queue,
    ) -> Self::Swapchain;

    fn resize(&self, swapchain: &mut Self::Swapchain, extent: [u32; 2]);

    fn destroy_swapchain_image(&self, image: <Self::Swapchain as Surface>::Texture);

    fn destroy_swapchain(&self, swapchain: Self::Swapchain);
}

pub trait Surface {
    type Texture;

    fn drain_frames(&mut self) -> impl Iterator<Item = SwapchainFrame<Self::Texture>>;
    fn next_frame_index(&mut self) -> usize;
    fn next_frame(&mut self) -> &mut SwapchainFrame<Self::Texture>;
    fn present(&self);
}

#[derive(Debug)]
pub struct SwapchainFrame<T> {
    pub texture: T,
    pub last_access: SyncPoint,
}
