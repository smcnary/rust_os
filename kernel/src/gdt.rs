//! Kernel code segment and a TSS so the double-fault handler has its own stack.

use spin::Lazy;
use x86_64::instructions::segmentation::{CS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

const STACK_LEN: usize = 4096 * 5;
pub const DOUBLE_FAULT_IST: u16 = 0;

#[repr(align(16))]
struct Stack([u8; STACK_LEN]);

static DOUBLE_FAULT_STACK: Stack = Stack([0; STACK_LEN]);

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    let start = VirtAddr::from_ptr(DOUBLE_FAULT_STACK.0.as_ptr());
    tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = start + STACK_LEN as u64;
    tss
});

struct Selectors {
    code: SegmentSelector,
    tss: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code = gdt.append(Descriptor::kernel_code_segment());
    let tss = gdt.append(Descriptor::tss_segment(&*TSS));
    (gdt, Selectors { code, tss })
});

pub fn init() {
    let gdt = &*GDT;
    gdt.0.load();
    unsafe {
        CS::set_reg(gdt.1.code);
        load_tss(gdt.1.tss);
    }
}
