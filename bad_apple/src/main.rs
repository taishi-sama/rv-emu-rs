#![no_main]
#![no_std]

use core::{panic::PanicInfo, fmt::{Write, self}, arch::asm};



#[inline(never)]
#[no_mangle]
pub fn main() {

    let mut uart = UART::new();
    let my_str = "";
    //let my_str = include_str!("play.txt");
    for (i, s) in my_str.split("SPLIT").enumerate(){
        writeln!(uart, "{}", i).unwrap();
        writeln!(uart, "{}", s).unwrap();

        for _ in 0..6100000 {
            unsafe{
                asm!("nop");
            }
        }
    }
    writeln!(uart, "{}", 1000).unwrap();   
}
#[no_mangle]
pub extern "C" fn __start() -> ! {
    main();
    loop {
        
    }
}

unsafe fn send_to_uart(byte: u8) {
    let t: usize = 0x10000000;
    let p = t as *mut u8;
    p.write_volatile(byte)
}

struct UART {}
impl UART {
    fn new()->Self{
        UART {  }
    }
}
impl fmt::Write for UART {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for i in s.as_bytes() {
            unsafe { send_to_uart(*i) };
        }
        Ok(())
    }
}



#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    //let t = b"Panic occured!";
    let mut uart = UART::new();
    writeln!(uart, "{}", _info).unwrap();
    loop {}
}
