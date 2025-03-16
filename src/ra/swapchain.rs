use crate::{
    collections::handle::Handle,
    rhi::swapchain::{Surface as _, SwapchainDesc, SwapchainFrame},
};

use super::{
    container::HandleContainer,
    context::{Context, RenderDevice},
    resources::Texture,
};

pub struct Swapchain<D: RenderDevice> {
    raw: D::Swapchain,
    frames: Vec<SwapchainFrame<Handle<Texture>>>,
    desc: SwapchainDesc,
}

pub trait RenderSwapchainContext {
    type Swapchain;
    type Wnd;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &Self::Wnd,
        handle_allocator: &HandleContainer,
    ) -> Self::Swapchain;
    fn resize(
        &self,
        swapchain: &mut Self::Swapchain,
        extent: [u32; 2],
        handle_allocator: &HandleContainer,
    );
    fn destroy_swapchain(&self, swapchain: Self::Swapchain, handle_allocator: &HandleContainer);
}

impl<D: RenderDevice> RenderSwapchainContext for Context<D> {
    type Swapchain = Swapchain<D>;
    type Wnd = D::Wnd;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &Self::Wnd,
        handle_allocator: &HandleContainer,
    ) -> Self::Swapchain {
        let handles = (0..desc.frames)
            .map(|_| handle_allocator.create_texture_handle())
            .collect::<Vec<_>>();

        let mut raw = self
            .gpu
            .create_swapchain(desc.clone(), wnd, &self.graphics_queue.raw);

        let frames = raw.drain_frames();

        for (frame, handle) in frames.into_iter().zip(handles.iter()) {
            self.mapper.textures.lock().set(*handle, frame.texture);
        }

        let frames = handles
            .into_iter()
            .map(|h| SwapchainFrame {
                texture: h,
                last_access: 0,
            })
            .collect();

        Swapchain { raw, frames, desc }
    }

    fn resize(
        &self,
        swapchain: &mut Self::Swapchain,
        extent: [u32; 2],
        handle_allocator: &HandleContainer,
    ) {
        for frame in swapchain.frames.drain(..) {
            handle_allocator.free_texture_handle(frame.texture);
        }

        self.gpu.resize(&mut swapchain.raw, extent);

        let handles = (0..swapchain.desc.frames)
            .map(|_| handle_allocator.create_texture_handle())
            .collect::<Vec<_>>();

        let frames = swapchain.raw.drain_frames();

        for (frame, handle) in frames.into_iter().zip(handles.iter()) {
            self.mapper.textures.lock().set(*handle, frame.texture);
        }

        let frames = handles
            .into_iter()
            .map(|h| SwapchainFrame {
                texture: h,
                last_access: 0,
            })
            .collect();

        swapchain.frames = frames;
    }

    fn destroy_swapchain(&self, swapchain: Self::Swapchain, handle_allocator: &HandleContainer) {
        self.gpu.destroy_swapchain(swapchain.raw);

        for frame in swapchain.frames {
            handle_allocator.free_texture_handle(frame.texture);
        }
    }
}

pub trait Surface {
    fn next_frame(&mut self) -> &SwapchainFrame<Handle<Texture>>;
    fn present(&self);
}

impl<D: RenderDevice> Surface for Swapchain<D> {
    fn next_frame(&mut self) -> &SwapchainFrame<Handle<Texture>> {
        let idx = self.raw.next_frame_index();
        &self.frames[idx]
    }

    fn present(&self) {
        self.raw.present();
    }
}
