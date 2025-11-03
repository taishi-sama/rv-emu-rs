#![no_main]
#![no_std]

extern crate alloc;

use core::{fmt::{self, Write}, panic::PanicInfo};
use alloc::vec::Vec;
use embedded_alloc::LlffHeap as Heap;

use riscv::{asm::delay, interrupt::Exception};
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
    //let my_str = "";
    let mut curr_time = riscv::register::time::read64();
    const MICROS: u64 = ((1.0 / TARGET_FPS as f64) * 1_000_000.0) as u64; 
    let total_samples_len = AUDIO_STREAM.len() as u32 / 2;
    let mut buff_iter = 0u32;
    for (i, s) in VIDEO_STREAM.split("SPLIT").enumerate(){
        // v.push(10);
        writeln!(uart, "{}, {} microseconds; Audio: {}/{}({:3.2}%) samples;", i, curr_time, buff_iter, total_samples_len, (buff_iter as f32) / (total_samples_len as f32) * 100.0).unwrap();
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
        while riscv::register::time::read64() - curr_time < MICROS {
            delay(100);
        }
        curr_time = riscv::register::time::read64();
        // if v.len() > 10000 {
            // v.clear();
        // }
        //riscv::asm::delay(10000000);
    }
    writeln!(uart, "FIN").unwrap();   
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
