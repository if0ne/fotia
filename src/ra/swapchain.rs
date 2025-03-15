use crate::{
    collections::handle::Handle,
    rhi::swapchain::{RenderSwapchainDevice, Surface, SwapchainDesc},
};

use super::{
    context::{Context, RenderDevice},
    resources::Texture,
};

pub struct Swapchain<D: RenderDevice> {
    raw: D::Swapchain,
    handles: Vec<Handle<Texture>>,
}

pub struct Frame<D: RenderDevice> {
    raw: <D::Swapchain as Surface>::Frame,

    pub handle: Handle<Texture>,
}

// TODO: Create RA trait for Swapchain
impl<D: RenderDevice> RenderSwapchainDevice for Context<D> {
    type Swapchain = Swapchain<D>;
    type Wnd = D::Wnd;
    type Queue = D::CommandQueue;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &Self::Wnd,
        queue: &Self::Queue,
    ) -> Self::Swapchain {
        todo!()
    }

    fn resize(&self, swapchain: &mut Self::Swapchain, extent: [u32; 2]) {
        todo!()
    }

    fn destroy_swapchain(&self, swapchain: Self::Swapchain) {
        todo!()
    }
}

impl<D: RenderDevice> Surface for Swapchain<D> {
    type Frame = Frame<D>;

    fn next_frame(&mut self) -> &Self::Frame {
        todo!()
    }

    fn present(&self) {
        self.raw.present();
    }
}
