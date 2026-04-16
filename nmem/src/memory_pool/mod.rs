use core::alloc::Layout;
use core::ptr::NonNull;

use ngpu::cmdbuf;
use ngpu::mem;

#[repr(C, align(8))]
pub struct RawNvnMemoryPoolBuilder {
    reserved: [u8; 64],
}

#[repr(C, align(8))]
pub struct RawNvnMemoryPool {
    reserved: [u8; 256],
}

#[repr(C, align(8))]
pub struct RawNvnCommandBuffer {
    reserved: [u8; 160],
}

#[derive(Debug)]
pub enum MemoryPoolError {
    InvalidSize,
    InvalidAlignment,
    AllocLayout,
    AllocFailed,
    DeviceNull,
    InitializeFailed,
    ArenaExhausted,
    CommandBufferInitFailed,
}

#[derive(Debug)]
struct AlignedStorage {
    ptr: NonNull<u8>,
    layout: Layout,
    size: usize,
}

impl AlignedStorage {
    fn new(size: usize, alignment: usize) -> Result<Self, MemoryPoolError> {
        if size == 0 {
            return Err(MemoryPoolError::InvalidSize);
        }
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(MemoryPoolError::InvalidAlignment);
        }
        let layout = Layout::from_size_align(size, alignment).map_err(|_| MemoryPoolError::AllocLayout)?;
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        let ptr = NonNull::new(ptr).ok_or(MemoryPoolError::AllocFailed)?;
        Ok(Self { ptr, layout, size })
    }

    #[inline(always)]
    fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for AlignedStorage {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr.as_ptr(), self.layout) };
    }
}

#[derive(Debug)]
pub struct PoolAllocation {
    pub offset: usize,
    pub size: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct CommandBufferBacking {
    pub command_offset: usize,
    pub command_size: usize,
    pub control_offset: usize,
    pub control_size: usize,
}

/// Simple aligned linear allocator over a memory pool byte-range.
///
/// This intentionally does not support free-list reuse yet; use `reset()` at
/// frame boundaries for "eager commit + reuse" behavior.
#[derive(Debug)]
pub struct LinearPoolAllocator {
    capacity: usize,
    cursor: usize,
}

impl LinearPoolAllocator {
    #[inline(always)]
    pub const fn new(capacity: usize) -> Self {
        Self { capacity, cursor: 0 }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    #[inline(always)]
    pub fn used(&self) -> usize {
        self.cursor
    }

    #[inline(always)]
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.cursor)
    }

    pub fn alloc(&mut self, size: usize, alignment: usize) -> Option<PoolAllocation> {
        if size == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }
        let aligned = (self.cursor + (alignment - 1)) & !(alignment - 1);
        let end = aligned.checked_add(size)?;
        if end > self.capacity {
            return None;
        }
        self.cursor = end;
        Some(PoolAllocation { offset: aligned, size })
    }
}

pub struct OwnedMemoryPool {
    storage: AlignedStorage,
    builder: Box<RawNvnMemoryPoolBuilder>,
    pool: Box<RawNvnMemoryPool>,
    device: *mut ngpu::NvnDevice,
    flags: i32,
    finalized: bool,
}

impl OwnedMemoryPool {
    /// Creates and initializes a new NVN memory pool with caller-owned backing storage.
    ///
    /// `alignment` should follow NVN storage rules (typically page-aligned).
    pub unsafe fn new(
        device: *mut ngpu::NvnDevice,
        size: usize,
        alignment: usize,
        flags: i32,
        label_nul: Option<&[u8]>,
    ) -> Result<Self, MemoryPoolError> {
        if device.is_null() {
            return Err(MemoryPoolError::DeviceNull);
        }

        let storage = AlignedStorage::new(size, alignment)?;
        let mut builder = Box::new(core::mem::zeroed::<RawNvnMemoryPoolBuilder>());
        let mut pool = Box::new(core::mem::zeroed::<RawNvnMemoryPool>());

        let builder_ptr = (&mut *builder) as *mut RawNvnMemoryPoolBuilder as *mut ngpu::NvnMemoryPoolBuilder;
        let pool_ptr = (&mut *pool) as *mut RawNvnMemoryPool as *mut ngpu::NvnMemoryPool;

        mem::memory_pool_builder_set_defaults(builder_ptr);
        mem::memory_pool_builder_set_device(builder_ptr, device);
        mem::memory_pool_builder_set_storage(builder_ptr, storage.as_mut_ptr().cast(), size);
        mem::memory_pool_builder_set_flags(builder_ptr, flags);
        if mem::memory_pool_initialize(pool_ptr, builder_ptr) == 0 {
            return Err(MemoryPoolError::InitializeFailed);
        }
        if let Some(label) = label_nul {
            if !label.is_empty() {
                mem::memory_pool_set_debug_label(pool_ptr, label.as_ptr());
            }
        }

        ncommon::logN!(
            target: "mem.pool",
            "initialized pool size=0x{:x} align=0x{:x} flags=0x{:x} pool={:p}",
            size,
            alignment,
            flags,
            pool_ptr
        );

        Ok(Self {
            storage,
            builder,
            pool,
            device,
            flags,
            finalized: false,
        })
    }

    #[inline(always)]
    pub fn as_raw_pool_ptr(&self) -> *mut ngpu::NvnMemoryPool {
        (&*self.pool as *const RawNvnMemoryPool).cast_mut().cast()
    }

    #[inline(always)]
    pub fn as_raw_builder_ptr(&self) -> *const ngpu::NvnMemoryPoolBuilder {
        (&*self.builder as *const RawNvnMemoryPoolBuilder).cast()
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.storage.size
    }

    #[inline(always)]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    #[inline(always)]
    pub fn device_ptr(&self) -> *mut ngpu::NvnDevice {
        self.device
    }

    #[inline(always)]
    pub unsafe fn map_ptr(&self) -> *mut u8 {
        mem::memory_pool_map(self.as_raw_pool_ptr()).cast()
    }

    #[inline(always)]
    pub unsafe fn flush_mapped_range(&self, offset: usize, size: usize) {
        mem::memory_pool_flush_mapped_range(self.as_raw_pool_ptr(), offset as isize, size);
    }

    #[inline(always)]
    pub unsafe fn invalidate_mapped_range(&self, offset: usize, size: usize) {
        mem::memory_pool_invalidate_mapped_range(self.as_raw_pool_ptr(), offset as isize, size);
    }

    #[inline(always)]
    pub unsafe fn buffer_address(&self) -> ngpu::NvnBufferAddress {
        mem::memory_pool_get_buffer_address(self.as_raw_pool_ptr())
    }

    pub unsafe fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        mem::memory_pool_finalize(self.as_raw_pool_ptr());
        self.finalized = true;
        ncommon::logN!(target: "mem.pool", "finalized pool={:p}", self.as_raw_pool_ptr());
    }
}

impl Drop for OwnedMemoryPool {
    fn drop(&mut self) {
        unsafe { self.finalize() };
    }
}

pub struct CommandBufferArena {
    pool: OwnedMemoryPool,
    command_alloc: LinearPoolAllocator,
    control_storage: AlignedStorage,
    control_alloc: LinearPoolAllocator,
    command_alignment: usize,
    control_alignment: usize,
}

impl CommandBufferArena {
    /// Creates an arena for NVN command buffer command/control memory.
    ///
    /// `pool_flags` should be chosen for command memory usage; command and
    /// control alignment can be tightened later once you query device info.
    pub unsafe fn new(
        device: *mut ngpu::NvnDevice,
        command_pool_size: usize,
        control_storage_size: usize,
        pool_flags: i32,
        pool_alignment: usize,
        command_alignment: usize,
        control_alignment: usize,
        label_nul: Option<&[u8]>,
    ) -> Result<Self, MemoryPoolError> {
        let pool = OwnedMemoryPool::new(
            device,
            command_pool_size,
            pool_alignment,
            pool_flags,
            label_nul,
        )?;
        let control_storage = AlignedStorage::new(control_storage_size, control_alignment)?;
        Ok(Self {
            pool,
            command_alloc: LinearPoolAllocator::new(command_pool_size),
            control_storage,
            control_alloc: LinearPoolAllocator::new(control_storage_size),
            command_alignment,
            control_alignment,
        })
    }

    #[inline(always)]
    pub fn pool(&self) -> &OwnedMemoryPool {
        &self.pool
    }

    #[inline(always)]
    pub fn pool_mut(&mut self) -> &mut OwnedMemoryPool {
        &mut self.pool
    }

    #[inline(always)]
    pub fn command_alignment(&self) -> usize {
        self.command_alignment
    }

    #[inline(always)]
    pub fn control_alignment(&self) -> usize {
        self.control_alignment
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.command_alloc.reset();
        self.control_alloc.reset();
    }

    pub fn allocate_backing(
        &mut self,
        command_size: usize,
        control_size: usize,
    ) -> Result<CommandBufferBacking, MemoryPoolError> {
        let cmd = self
            .command_alloc
            .alloc(command_size, self.command_alignment)
            .ok_or(MemoryPoolError::ArenaExhausted)?;
        let ctl = self
            .control_alloc
            .alloc(control_size, self.control_alignment)
            .ok_or(MemoryPoolError::ArenaExhausted)?;
        Ok(CommandBufferBacking {
            command_offset: cmd.offset,
            command_size: cmd.size,
            control_offset: ctl.offset,
            control_size: ctl.size,
        })
    }

    #[inline(always)]
    fn control_ptr_at(&self, offset: usize) -> *mut u8 {
        unsafe { self.control_storage.as_mut_ptr().add(offset) }
    }
}

pub struct OwnedCommandBuffer {
    raw: Box<RawNvnCommandBuffer>,
    backing: CommandBufferBacking,
    finalized: bool,
}

impl OwnedCommandBuffer {
    /// Initializes a command buffer and wires command/control backing from `arena`.
    pub unsafe fn new(
        arena: &mut CommandBufferArena,
        device: *mut ngpu::NvnDevice,
        command_size: usize,
        control_size: usize,
        label_nul: Option<&[u8]>,
    ) -> Result<Self, MemoryPoolError> {
        let backing = arena.allocate_backing(command_size, control_size)?;
        let mut raw = Box::new(core::mem::zeroed::<RawNvnCommandBuffer>());
        let raw_ptr = (&mut *raw as *mut RawNvnCommandBuffer).cast::<ngpu::NvnCommandBuffer>();

        if cmdbuf::command_buffer_initialize(raw_ptr, device) == 0 {
            return Err(MemoryPoolError::CommandBufferInitFailed);
        }
        cmdbuf::command_buffer_add_command_memory(
            raw_ptr,
            arena.pool().as_raw_pool_ptr(),
            backing.command_offset as isize,
            backing.command_size,
        );
        cmdbuf::command_buffer_add_control_memory(
            raw_ptr,
            arena.control_ptr_at(backing.control_offset).cast(),
            backing.control_size,
        );
        if let Some(label) = label_nul {
            if !label.is_empty() {
                cmdbuf::command_buffer_set_debug_label(raw_ptr, label.as_ptr());
            }
        }

        ncommon::logN!(
            target: "mem.cmdbuf",
            "initialized cmdbuf={:p} cmd(off=0x{:x},sz=0x{:x}) ctl(off=0x{:x},sz=0x{:x})",
            raw_ptr,
            backing.command_offset,
            backing.command_size,
            backing.control_offset,
            backing.control_size
        );

        Ok(Self {
            raw,
            backing,
            finalized: false,
        })
    }

    #[inline(always)]
    pub fn as_raw_ptr(&self) -> *mut ngpu::NvnCommandBuffer {
        (&*self.raw as *const RawNvnCommandBuffer).cast_mut().cast()
    }

    #[inline(always)]
    pub fn backing(&self) -> CommandBufferBacking {
        self.backing
    }

    #[inline(always)]
    pub unsafe fn begin_recording(&self) {
        cmdbuf::command_buffer_begin_recording(self.as_raw_ptr());
    }

    #[inline(always)]
    pub unsafe fn rebind_recording_memory(&self, arena: &CommandBufferArena) {
        // NVN command buffers consume command/control memory each recording;
        // re-add the backing slices before each BeginRecording, mirroring starlight.
        cmdbuf::command_buffer_add_command_memory(
            self.as_raw_ptr(),
            arena.pool().as_raw_pool_ptr(),
            self.backing.command_offset as isize,
            self.backing.command_size,
        );
        cmdbuf::command_buffer_add_control_memory(
            self.as_raw_ptr(),
            arena.control_ptr_at(self.backing.control_offset).cast(),
            self.backing.control_size,
        );
    }

    #[inline(always)]
    pub unsafe fn end_recording(&self) -> ngpu::NvnCommandHandle {
        cmdbuf::command_buffer_end_recording(self.as_raw_ptr())
    }

    pub unsafe fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        cmdbuf::command_buffer_finalize(self.as_raw_ptr());
        self.finalized = true;
    }
}

impl Drop for OwnedCommandBuffer {
    fn drop(&mut self) {
        unsafe { self.finalize() };
    }
}
