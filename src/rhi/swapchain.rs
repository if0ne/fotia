use super::command::RenderCommandDevice;

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
    type Wnd;
    type Queue;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &Self::Wnd,
        queue: &Self::Queue,
    ) -> Self::Swapchain;

    fn resize(&self, swapchain: &mut Self::Swapchain, extent: [u32; 2]);

    fn destroy_swapchain(&self, swapchain: Self::Swapchain);
}

pub trait Surface {
    type Frame;

    fn next_frame(&mut self) -> &Self::Frame;
    fn present(&self);
}
