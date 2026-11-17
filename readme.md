

# What is it.
My small personal project targeted to write RISC-V emulator with decent accuracy.

Current target instruction set is rv32imafdc, with Sv32 virtual memory and user mode - essentially full RV32GC.


https://github.com/user-attachments/assets/7547b81b-a22c-4d94-8c82-70087b4962d8


Devices list contains a 16550-compatible UART, a write-only audio device with fixed rate and format, a simple 640x480 framebuffer, a CLINT and a syscon. Will be extended in the future.


# Credits 
## Emulation
Project was heavily


 inspired by [mini-rv32ima emulator](https://github.com/cnlohr/mini-rv32ima).

elf_analyzer.rs and some parts of code was took from [riscv-rust emulator](https://github.com/d0iasm/rvemu). Corresponding license is located in OTHER_LICENSES folder.


## Bad Apple example
Rights to "Bad Apple!!" is owned by Alstroemeria Records. 
bad_apple/Bad Apple!!.ogg was ripped from [this video](https://www.youtube.com/watch?v=FtutLA63Cp8).

Thanks to [Chion82](https://github.com/Chion82/ASCII_bad_apple) for their ASCII-conversion of video clip.

