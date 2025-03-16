use smallvec::SmallVec;
use winit::raw_window_handle::RawWindowHandle;

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
    frames: SmallVec<[SwapchainFrame<Handle<Texture>>; 4]>,
    desc: SwapchainDesc,
}

pub trait RenderSwapchainContext {
    type Swapchain;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &RawWindowHandle,
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

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &RawWindowHandle,
        handle_allocator: &HandleContainer,
    ) -> Self::Swapchain {
        let handles = (0..desc.frames).map(|_| handle_allocator.create_texture_handle());

        let mut raw = self
            .gpu
            .create_swapchain(desc.clone(), wnd, &self.graphics_queue.raw);

        let frames = raw.drain_frames();

        let frames = frames
            .into_iter()
            .zip(handles)
            .map(|(frame, handle)| {
                self.mapper.textures.lock().set(handle, frame.texture);

                SwapchainFrame {
                    texture: handle,
                    last_access: 0,
                }
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
            if let Some(texture) = self.mapper.textures.lock().remove(frame.texture) {
                self.gpu.destroy_swapchain_image(texture);
            };

            handle_allocator.free_texture_handle(frame.texture);
        }

        self.gpu.resize(&mut swapchain.raw, extent);

        let handles = (0..swapchain.desc.frames).map(|_| handle_allocator.create_texture_handle());

        let frames = swapchain.raw.drain_frames();

        let frames = frames
            .into_iter()
            .zip(handles)
            .map(|(frame, handle)| {
                self.mapper.textures.lock().set(handle, frame.texture);

                SwapchainFrame {
                    texture: handle,
                    last_access: 0,
                }
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
    fn next_frame(&mut self) -> &mut SwapchainFrame<Handle<Texture>>;
    fn present(&self);
}

impl<D: RenderDevice> Surface for Swapchain<D> {
    fn next_frame(&mut self) -> &mut SwapchainFrame<Handle<Texture>> {
        let idx = self.raw.next_frame_index();
        &mut self.frames[idx]
    }

    fn present(&self) {
        self.raw.present();
    }
}
