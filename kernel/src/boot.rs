//! Limine boot-protocol requests. The bootloader finds these by magic
//! numbers placed between the start and end markers in `.data`.

use core::ptr;

const COMMON_0: u64 = 0xc7b1dd30df4c8b88;
const COMMON_1: u64 = 0x0a82e883a194f07b;

#[used]
#[link_section = ".requests_start"]
static START_MARKER: [u64; 4] = [
    0xf6b8f4b39de7d1ae,
    0xfab91a6940fcb9cf,
    0x785c6ed015d3e316,
    0x181e920a7852b9d9,
];

#[used]
#[link_section = ".requests_end"]
static END_MARKER: [u64; 2] = [0xadc0e0531bb10d03, 0x9572709f31764c62];

/// Requested base revision. Limine sets `tag[2]` to 0 when it honours it.
#[used]
#[link_section = ".requests"]
static mut BASE_REVISION: [u64; 3] = [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 3];

#[repr(C)]
struct Request {
    id: [u64; 4],
    revision: u64,
    response: *mut u8,
}

#[repr(C)]
struct StackRequest {
    id: [u64; 4],
    revision: u64,
    response: *mut u8,
    stack_size: u64,
}

#[used]
#[link_section = ".requests"]
static mut FRAMEBUFFER: Request = Request {
    id: [COMMON_0, COMMON_1, 0x9d5827dcd881dd75, 0xa3148604f6fab11b],
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".requests"]
static mut MEMMAP: Request = Request {
    id: [COMMON_0, COMMON_1, 0x67cf3d9d378a806f, 0xe304acdfc50c3c62],
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".requests"]
static mut HHDM: Request = Request {
    id: [COMMON_0, COMMON_1, 0x48dcf1cb8ad2b852, 0x63984e959a98244b],
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".requests"]
static mut STACK: StackRequest = StackRequest {
    id: [COMMON_0, COMMON_1, 0x224ef0460a8e8926, 0xe1cb0fc25f46ea3d],
    revision: 0,
    response: ptr::null_mut(),
    stack_size: 64 * 1024,
};

#[repr(C)]
pub struct Framebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

#[repr(C)]
struct FramebufferResponse {
    revision: u64,
    count: u64,
    framebuffers: *const *const Framebuffer,
}

#[repr(C)]
pub struct MemmapEntry {
    pub base: u64,
    pub length: u64,
    pub kind: u64,
}

pub const MEMMAP_USABLE: u64 = 0;

#[repr(C)]
struct MemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *const *const MemmapEntry,
}

#[repr(C)]
struct HhdmResponse {
    revision: u64,
    offset: u64,
}

/// Touch every request so the linker keeps the magic numbers.
pub fn revision_supported() -> bool {
    let _ = &START_MARKER;
    let _ = &END_MARKER;
    // The bootloader writes the tag before jumping here. A plain read can be
    // folded to the compile-time value, so this has to be volatile.
    let tag = unsafe { ptr::read_volatile(ptr::addr_of!(BASE_REVISION)) };
    let _ = unsafe { ptr::read_volatile(ptr::addr_of!(STACK)) };
    tag[2] == 0
}

pub fn framebuffer() -> Option<&'static Framebuffer> {
    let response = response_ptr(ptr::addr_of!(FRAMEBUFFER))?;
    let response = unsafe { &*response.cast::<FramebufferResponse>() };
    if response.count == 0 || response.framebuffers.is_null() {
        return None;
    }
    let first = unsafe { *response.framebuffers };
    if first.is_null() {
        None
    } else {
        Some(unsafe { &*first })
    }
}

pub fn hhdm_offset() -> Option<u64> {
    let response = response_ptr(ptr::addr_of!(HHDM))?;
    let response = unsafe { &*response.cast::<HhdmResponse>() };
    Some(response.offset)
}

pub fn usable_regions() -> impl Iterator<Item = (u64, u64)> {
    let response = response_ptr(ptr::addr_of!(MEMMAP));
    let (entries, count) = if let Some(response) = response {
        let response = unsafe { &*response.cast::<MemmapResponse>() };
        (response.entries, response.entry_count as usize)
    } else {
        (ptr::null(), 0)
    };
    (0..count).filter_map(move |index| {
        if entries.is_null() {
            return None;
        }
        let entry = unsafe { *entries.add(index) };
        if entry.is_null() {
            return None;
        }
        let entry = unsafe { &*entry };
        if entry.kind == MEMMAP_USABLE && entry.length > 0 {
            Some((entry.base, entry.length))
        } else {
            None
        }
    })
}

fn response_ptr(request: *const Request) -> Option<*mut u8> {
    let request = unsafe { ptr::read_volatile(request) };
    if request.response.is_null() {
        None
    } else {
        Some(request.response)
    }
}
