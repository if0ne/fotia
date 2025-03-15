use std::ops::Range;

use oxidx::dx::{self, IDescriptorHeap, IDevice};
use parking_lot::Mutex;
use tracing::info;

use crate::rhi::backend::RenderDeviceInfo;

#[derive(Debug)]
pub struct DxDevice {
    _adapter: dx::Adapter3,

    pub(super) factory: dx::Factory4,
    pub(super) gpu: dx::Device,
    pub(super) desc: RenderDeviceInfo,
    pub(super) descriptors: DescriptorPool,
}

impl DxDevice {
    pub(super) fn new(
        adapter: dx::Adapter3,
        factory: dx::Factory4,
        desc: RenderDeviceInfo,
    ) -> Self {
        info!(
            "Creating device with adapter {} and id {}",
            desc.name, desc.id
        );

        let device = dx::create_device(Some(&adapter), dx::FeatureLevel::Level11)
            .expect("failed to create device");

        if desc.is_cross_adapter_texture_supported {
            info!("Cross Adapter Row Major Texture is supported");
        } else {
            info!("Cross Adapter Row Major Texture is NOT supported");
        }

        let descriptors = DescriptorPool::new(&device);

        Self {
            gpu: device,
            _adapter: adapter,
            factory,
            desc,
            descriptors,
        }
    }
}

#[derive(Debug)]
pub(super) struct DescriptorPool {
    pub(super) rtv_heap: Mutex<DescriptorHeap>,
    pub(super) dsv_heap: Mutex<DescriptorHeap>,
    pub(super) shader_heap: Mutex<DescriptorHeap>,
    pub(super) sampler_heap: Mutex<DescriptorHeap>,
}

impl DescriptorPool {
    // TODO: size configuration
    fn new(device: &dx::Device) -> Self {
        let rtv_heap = DescriptorHeap::new(&device, dx::DescriptorHeapType::Rtv, 128);
        let dsv_heap = DescriptorHeap::new(&device, dx::DescriptorHeapType::Dsv, 128);
        let shader_heap = DescriptorHeap::new(&device, dx::DescriptorHeapType::CbvSrvUav, 1024);
        let sampler_heap = DescriptorHeap::new(&device, dx::DescriptorHeapType::Sampler, 32);

        Self {
            rtv_heap: Mutex::new(rtv_heap),
            dsv_heap: Mutex::new(dsv_heap),
            shader_heap: Mutex::new(shader_heap),
            sampler_heap: Mutex::new(sampler_heap),
        }
    }

    pub(super) fn allocate(&self, ty: dx::DescriptorHeapType, size: usize) -> Descriptor {
        match ty {
            dx::DescriptorHeapType::Rtv => self.rtv_heap.lock().allocate(size),
            dx::DescriptorHeapType::Dsv => self.dsv_heap.lock().allocate(size),
            dx::DescriptorHeapType::CbvSrvUav => self.shader_heap.lock().allocate(size),
            dx::DescriptorHeapType::Sampler => self.sampler_heap.lock().allocate(size),
        }
    }

    pub(super) fn free(&self, descriptor: Descriptor) {
        match descriptor.ty {
            dx::DescriptorHeapType::Rtv => self.rtv_heap.lock().free(descriptor),
            dx::DescriptorHeapType::Dsv => self.dsv_heap.lock().free(descriptor),
            dx::DescriptorHeapType::CbvSrvUav => self.shader_heap.lock().free(descriptor),
            dx::DescriptorHeapType::Sampler => self.sampler_heap.lock().free(descriptor),
        }
    }
}

#[derive(Debug)]
pub(super) struct DescriptorHeap {
    ty: dx::DescriptorHeapType,
    _size: usize,
    shader_visible: bool,
    allocator: range_alloc::RangeAllocator<usize>,

    pub(super) heap: dx::DescriptorHeap,
    pub(super) inc_size: usize,
}

impl DescriptorHeap {
    fn new(device: &dx::Device, ty: dx::DescriptorHeapType, size: usize) -> Self {
        let (shader_visible, flags) =
            if ty == dx::DescriptorHeapType::CbvSrvUav || ty == dx::DescriptorHeapType::Sampler {
                (true, dx::DescriptorHeapFlags::ShaderVisible)
            } else {
                (false, dx::DescriptorHeapFlags::empty())
            };

        let inc_size = device.get_descriptor_handle_increment_size(ty);

        let heap = device
            .create_descriptor_heap(&dx::DescriptorHeapDesc::new(ty, size).with_flags(flags))
            .expect("Failed to create descriptor heap");

        let allocator = range_alloc::RangeAllocator::new(0..size);

        Self {
            heap,
            ty,
            _size: size,
            inc_size,
            shader_visible,
            allocator,
        }
    }

    pub(super) fn allocate(&mut self, size: usize) -> Descriptor {
        let allocation = self
            .allocator
            .allocate_range(size)
            .expect("out of memory in descriptor heap");

        let cpu = self
            .heap
            .get_cpu_descriptor_handle_for_heap_start()
            .advance(allocation.start, self.inc_size);
        let gpu = if self.shader_visible {
            self.heap
                .get_gpu_descriptor_handle_for_heap_start()
                .advance(allocation.start, self.inc_size)
        } else {
            dx::GpuDescriptorHandle::default()
        };

        Descriptor {
            ty: self.ty,
            allocation,
            cpu,
            gpu,
        }
    }

    pub(super) fn free(&mut self, descriptor: Descriptor) {
        self.allocator.free_range(descriptor.allocation);
    }
}

#[derive(Debug)]
pub(super) struct Descriptor {
    ty: dx::DescriptorHeapType,
    allocation: Range<usize>,
    pub(super) cpu: dx::CpuDescriptorHandle,
    pub(super) gpu: dx::GpuDescriptorHandle,
}
