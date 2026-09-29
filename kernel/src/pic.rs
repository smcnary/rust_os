//! Legacy dual 8259, remapped so IRQs sit above the CPU exceptions.

use x86_64::instructions::port::Port;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const EOI: u8 = 0x20;

pub fn init() {
    unsafe {
        let mut cmd1 = Port::<u8>::new(PIC1_CMD);
        let mut data1 = Port::<u8>::new(PIC1_DATA);
        let mut cmd2 = Port::<u8>::new(PIC2_CMD);
        let mut data2 = Port::<u8>::new(PIC2_DATA);

        cmd1.write(0x11);
        wait();
        cmd2.write(0x11);
        wait();
        data1.write(0x20);
        wait();
        data2.write(0x28);
        wait();
        data1.write(4);
        wait();
        data2.write(2);
        wait();
        data1.write(0x01);
        wait();
        data2.write(0x01);
        wait();
        // IRQ0 timer and IRQ1 keyboard on the master. Mask everything else.
        data1.write(0b1111_1100);
        data2.write(0b1111_1111);
    }
}

pub fn end_of_interrupt(irq: u8) {
    unsafe {
        if irq >= 8 {
            Port::<u8>::new(PIC2_CMD).write(EOI);
        }
        Port::<u8>::new(PIC1_CMD).write(EOI);
    }
}

fn wait() {
    unsafe {
        Port::<u8>::new(0x80).write(0);
    }
}
