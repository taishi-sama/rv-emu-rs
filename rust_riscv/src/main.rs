#![no_main]
#![no_std]

use core::{panic::PanicInfo, fmt::Write, hint::{self, black_box}};
#[inline(never)]
#[no_mangle]
pub fn main() {

    let mut uart = UART::new();

    let a = black_box(100);
    let b = black_box(10);

    let d = a / b;
    let r = a % b;
    writeln!(uart, "{} {}", d, r).unwrap();

    let x = black_box(10);
    let y = black_box(10);
    let d = x / y;
    let r = x % y;
    writeln!(uart, "{} {}", d, r).unwrap();

    //writeln!(uart, "{}", 1000).unwrap();
    
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
impl Write for UART {
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
