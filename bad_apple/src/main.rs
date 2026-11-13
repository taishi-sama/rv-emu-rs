#![no_main]
#![no_std]

extern crate alloc;

use core::{fmt::{self, Write}, panic::PanicInfo};
use alloc::vec::Vec;
use embedded_alloc::LlffHeap as Heap;

use riscv::{asm::wfi, interrupt::Exception};
use riscv_rt::{core_interrupt, exception};

extern "C" {
    static _heap_size: u8;
}

#[global_allocator]
static HEAP: Heap = Heap::empty();


#[exception(Exception::MachineEnvCall)]
unsafe fn custom_menv_call_handler(_trap_frame: &riscv_rt::TrapFrame) {
    let mut uart = UART::new();
    // let r = riscv::register::mepc::read();
    // writeln!(uart, "Hello from 0x{:0x} location; Registers: {:0x}", r, _trap_frame.ra).unwrap();
}



const AUDIO_STREAM: &[u8] = include_bytes!("../raw_audio_stream.bin");
const VIDEO_STREAM: &str = include_str!("play.txt");

const TARGET_FPS: u32 = 10;

#[riscv_rt::entry]
fn main() -> ! {
    // Depends on implemented csrrci to disable interrupts for critical section. Can be hacked upon, but this will be wrong.
    // {
    //     let heap_bottom = riscv_rt::heap_start() as usize;
    //     unsafe { 
    //         let heap_size = &_heap_size as *const u8 as usize;
    //         HEAP.init(heap_bottom, heap_size) 
    //     }
    // }
    // let mut v = Vec::new();
    // for i in 0..10 {
    //     v.push(i);
    // }

    let mut uart = UART::new();
    let audio = Audio::new();
    const MICROS: u64 = ((1.0 / TARGET_FPS as f64) * 1_000_000.0) as u64; 
    let total_samples_len = AUDIO_STREAM.len() as u32 / 2;
    let mut buff_iter = 0u32;

    //MTIE lets WFI wake on the timer; mstatus.MIE stays clear so no trap is taken.
    unsafe {
        riscv::register::mstatus::clear_mie();
        riscv::register::mie::set_mtimer();
    }
    let mut deadline = riscv::register::time::read64();

    for (i, s) in VIDEO_STREAM.split("SPLIT").enumerate(){
        // v.push(10);
        let now = riscv::register::time::read64();
        writeln!(uart, "{}, {} microseconds; Audio: {}/{}({:3.2}%) samples;", i, now, buff_iter, total_samples_len, (buff_iter as f32) / (total_samples_len as f32) * 100.0).unwrap();
        writeln!(uart, "{}", s).unwrap();
        {
            let len = audio.read_len();
            let to_read = (44100u32 / 2 * 3).saturating_sub(len);
            let max_len = total_samples_len.saturating_sub(buff_iter);

            for _i in 0 .. to_read.min(max_len) {
                
                let sample = [AUDIO_STREAM[(buff_iter * 2) as usize], AUDIO_STREAM[(buff_iter * 2 + 1) as usize]];
                let sample = i16::from_le_bytes(sample);

                audio.write_sample(sample);
                buff_iter = buff_iter.wrapping_add(1);
            }
        }
        //Deadline based, so a slow frame does not shift the schedule.
        deadline = deadline.wrapping_add(MICROS);
        unsafe { set_mtimecmp(deadline) };
        //WFI wakes once MTIP is pending; an already-missed deadline returns immediately.
        while riscv::register::time::read64() < deadline {
            wfi();
        }
        // if v.len() > 10000 {
            // v.clear();
        // }
        //riscv::asm::delay(10000000);
    }
    writeln!(uart, "FIN").unwrap();
    //No pending timer should be able to trap if a future change enables global interrupts.
    unsafe { riscv::register::mie::clear_mtimer() };
    loop {
        
    }
}

//#[no_mangle]
//pub extern "C" fn __start() -> ! {
//    main();
//    loop {
//        
//    }
//}

const AUDIO_LOCATION: u32 = 0x10000200;

const CLINT_BASE: usize = 0x1100_0000;
const MTIMECMP_LOW: usize = CLINT_BASE + 0x4000;
const MTIMECMP_HIGH: usize = CLINT_BASE + 0x4004;

//Programs the next timer deadline. The high word is disarmed first so a carry into it cannot
//produce a transient compare value below mtime.
unsafe fn set_mtimecmp(deadline: u64) {
    let low = deadline as u32;
    let high = (deadline >> 32) as u32;
    (MTIMECMP_HIGH as *mut u32).write_volatile(u32::MAX);
    (MTIMECMP_LOW as *mut u32).write_volatile(low);
    (MTIMECMP_HIGH as *mut u32).write_volatile(high);
}

struct Audio {} 
impl Audio {
    pub fn new() -> Self {
        Audio {  }
    }
    pub fn read_len(&self) -> u32 {
        unsafe {
            let p = AUDIO_LOCATION as *const u32; 
            p.read_volatile()
        }
    }
    pub fn write_sample(&self, sample: i16) {
        unsafe {
            let p = AUDIO_LOCATION as *mut i16;
            p.write_volatile(sample)
        }
    } 
}
unsafe fn send_to_uart(byte: u8) {
    let t: usize = 0x10000000;
    let p = t as *mut u8;
    p.write_volatile(byte)
}

struct UART {}
impl UART {
    pub fn new()->Self{
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
