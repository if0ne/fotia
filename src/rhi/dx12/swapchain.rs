use std::num::NonZero;

use oxidx::dx::{self, IDevice, IFactory4, ISwapchain1, ISwapchain3};
use parking_lot::Mutex;

use crate::rhi::{
    resources::{TextureDesc, TextureType, TextureUsages, TextureViewDesc, TextureViewType},
    swapchain::{PresentMode, RenderSwapchainDevice, Surface, SwapchainDesc, SwapchainFrame},
    types::Format,
};

use super::{
    command::DxCommandQueue,
    device::DxDevice,
    resources::{DxTexture, TextureFlavor},
};

#[derive(Debug)]
pub struct Swapchain {
    raw: dx::Swapchain3,
    _hwnd: NonZero<isize>,
    resources: Vec<SwapchainFrame<DxTexture>>,
    desc: SwapchainDesc,
}

impl RenderSwapchainDevice for DxDevice {
    type Swapchain = Swapchain;
    type Wnd = NonZero<isize>;
    type Queue = DxCommandQueue;

    fn create_swapchain(
        &self,
        desc: SwapchainDesc,
        wnd: &Self::Wnd,
        queue: &Self::Queue,
    ) -> Self::Swapchain {
        let width = desc.width;
        let height = desc.height;

        let raw_desc = dx::SwapchainDesc1::new(desc.width, desc.height)
            .with_format(dx::Format::Rgba8Unorm)
            .with_usage(dx::FrameBufferUsage::RenderTargetOutput)
            .with_buffer_count(desc.frames)
            .with_scaling(dx::Scaling::None)
            .with_swap_effect(dx::SwapEffect::FlipDiscard)
            .with_flags(dx::SwapchainFlags::AllowTearing);

        let swapchain = self
            .factory
            .create_swapchain_for_hwnd(&*queue.queue.lock(), *wnd, &raw_desc, None, dx::OUTPUT_NONE)
            .expect("failed to create swapchain");

        let mut swapchain = Self::Swapchain {
            raw: swapchain.try_into().expect("failed to cast to Swapchain3"),
            _hwnd: *wnd,
            resources: vec![],
            desc,
        };
        self.resize(&mut swapchain, [width, height]);

        swapchain
    }

    fn resize(&self, swapchain: &mut Self::Swapchain, extent: [u32; 2]) {
        {
            let resources = std::mem::take(&mut swapchain.resources);

            let mut guard = self.descriptors.rtv_heap.lock();

            for res in resources {
                if let Some(descriptor) = res.texture.descriptor {
                    guard.free(descriptor);
                }
            }
        }

        swapchain
            .raw
            .resize_buffers(
                swapchain.desc.frames,
                extent[0],
                extent[1],
                dx::Format::Unknown,
                dx::SwapchainFlags::AllowTearing,
            )
            .expect("Failed to resize swapchain");

        for i in 0..swapchain.desc.frames {
            let res: dx::Resource = swapchain
                .raw
                .get_buffer(i)
                .expect("Failed to get swapchain buffer");

            let descriptor = self.descriptors.rtv_heap.lock().allocate(1);
            self.gpu
                .create_render_target_view(Some(&res), None, descriptor.cpu);
            let descriptor = Some(descriptor);

            let texture = DxTexture {
                raw: res,
                flavor: TextureFlavor::Local,
                desc: TextureDesc {
                    name: None,
                    ty: TextureType::D2,
                    mip_levels: 1,
                    format: Format::Rgba8Unorm,
                    usage: TextureUsages::RenderTarget,
                    extent: [extent[0], extent[1], 1],
                },
                size: 0, // TODO: Calculate
                descriptor,
                view: TextureViewDesc::default().with_view_type(TextureViewType::RenderTarget),
                _is_view: false,
                state: Mutex::new(dx::ResourceStates::Common),
            };

            swapchain.resources.push(SwapchainFrame {
                texture,
                last_access: 0,
            });
        }
    }

    fn destroy_swapchain(&self, mut swapchain: Self::Swapchain) {
        let resources = std::mem::take(&mut swapchain.resources);

        let mut guard = self.descriptors.rtv_heap.lock();

        for res in resources {
            if let Some(descriptor) = res.texture.descriptor {
                guard.free(descriptor);
            }
        }
    }
}

impl Surface for Swapchain {
    type Texture = DxTexture;

    fn drain_frames(&mut self) -> Vec<SwapchainFrame<Self::Texture>> {
        self.resources.drain(..).collect()
    }

    fn next_frame_index(&mut self) -> usize {
        self.raw.get_current_back_buffer_index() as usize
    }

    fn next_frame(&mut self) -> &SwapchainFrame<Self::Texture> {
        let next_idx = self.raw.get_current_back_buffer_index() as usize;
        &self.resources[next_idx]
    }

    fn present(&self) {
        let (interval, flags) = match self.desc.present_mode {
            PresentMode::Immediate => (0, dx::PresentFlags::AllowTearing),
            PresentMode::Mailbox => (0, dx::PresentFlags::empty()),
            PresentMode::Fifo => (1, dx::PresentFlags::empty()),
        };

        self.raw
            .present(interval, flags)
            .expect("failed to present");
    }
}
