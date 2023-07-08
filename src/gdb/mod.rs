use std::{net::{TcpStream, TcpListener}, io};

use gdbstub::{target::{
    ext::base::{
        singlethread::{SingleThreadBase, SingleThreadResume, SingleThreadSingleStep},
        BaseOps,
    },
    Target, TargetError,
}, stub::{run_blocking::{BlockingEventLoop, Event, WaitForStopReasonError}, SingleThreadStopReason, GdbStub}, conn::Connection, common::Signal};

use crate::{
    cpu::CPU,
    emulator::Emulator,
    traps::{FatalError, Trap},
};

impl Target for Emulator {
    type Arch = gdbstub_arch::riscv::Riscv32;
    fn base_ops(&mut self) -> BaseOps<Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }

    type Error = FatalError;

    fn guard_rail_implicit_sw_breakpoints(&self) -> bool {
        true
    }

    fn guard_rail_single_step_gdb_behavior(&self) -> gdbstub::arch::SingleStepGdbBehavior {
        <Self::Arch as gdbstub::arch::Arch>::single_step_gdb_behavior()
    }

    fn use_no_ack_mode(&self) -> bool {
        true
    }

    fn use_x_upcase_packet(&self) -> bool {
        true
    }

    fn use_resume_stub(&self) -> bool {
        true
    }

    fn use_rle(&self) -> bool {
        true
    }

    fn use_target_description_xml(&self) -> bool {
        true
    }

    fn use_lldb_register_info(&self) -> bool {
        true
    }

    fn support_breakpoints(
        &mut self,
    ) -> Option<gdbstub::target::ext::breakpoints::BreakpointsOps<'_, Self>> {
        None
    }

    fn support_monitor_cmd(
        &mut self,
    ) -> Option<gdbstub::target::ext::monitor_cmd::MonitorCmdOps<'_, Self>> {
        None
    }

    fn support_extended_mode(
        &mut self,
    ) -> Option<gdbstub::target::ext::extended_mode::ExtendedModeOps<'_, Self>> {
        None
    }

    fn support_section_offsets(
        &mut self,
    ) -> Option<gdbstub::target::ext::section_offsets::SectionOffsetsOps<'_, Self>> {
        None
    }

    fn support_target_description_xml_override(
        &mut self,
    ) -> Option<
        gdbstub::target::ext::target_description_xml_override::TargetDescriptionXmlOverrideOps<
            '_,
            Self,
        >,
    > {
        None
    }

    fn support_lldb_register_info_override(
        &mut self,
    ) -> Option<
        gdbstub::target::ext::lldb_register_info_override::LldbRegisterInfoOverrideOps<'_, Self>,
    > {
        None
    }

    fn support_memory_map(
        &mut self,
    ) -> Option<gdbstub::target::ext::memory_map::MemoryMapOps<'_, Self>> {
        None
    }

    fn support_catch_syscalls(
        &mut self,
    ) -> Option<gdbstub::target::ext::catch_syscalls::CatchSyscallsOps<'_, Self>> {
        None
    }

    fn support_host_io(&mut self) -> Option<gdbstub::target::ext::host_io::HostIoOps<'_, Self>> {
        None
    }

    fn support_exec_file(
        &mut self,
    ) -> Option<gdbstub::target::ext::exec_file::ExecFileOps<'_, Self>> {
        None
    }

    fn support_auxv(&mut self) -> Option<gdbstub::target::ext::auxv::AuxvOps<'_, Self>> {
        None
    }
}
impl SingleThreadBase for Emulator {
    fn support_single_register_access(
        &mut self,
    ) -> Option<
        gdbstub::target::ext::base::single_register_access::SingleRegisterAccessOps<'_, (), Self>,
    > {
        None
    }

    fn support_resume(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::singlethread::SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }

    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        regs.x = self.cpu.get_registers();
        regs.pc = self.cpu.pc;
        Ok(())
    }

    fn write_registers(
        &mut self,
        regs: &<Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        self.cpu.set_registers(regs.x);
        self.cpu.pc = regs.pc;
        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &mut [u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        for (address, value) in (start_addr..).zip(data.iter_mut()) {
            *value = match self.cpu.mmu.read_raw_from_ram(address) {
                Some(v) => v,
                None => {
                    return Err(TargetError::NonFatal);
                }
            };
        }
        Ok(())
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &[u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        for (address, value) in (start_addr..).zip(data.iter().copied()) {
            if !self.cpu.mmu.write_raw_to_ram(address, value){
                return Err(TargetError::NonFatal);
            };
        }
        Ok(())
    }
}

impl SingleThreadResume for Emulator {
    fn resume(&mut self, signal: Option<gdbstub::common::Signal>) -> Result<(), Self::Error> {
        todo!()
    }

    fn support_single_step(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::singlethread::SingleThreadSingleStepOps<'_, Self>> {
        Some(self)
    }

    fn support_range_step(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::singlethread::SingleThreadRangeSteppingOps<'_, Self>>
    {
        None
    }

    fn support_reverse_step(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::reverse_exec::ReverseStepOps<'_, (), Self>> {
        None
    }

    fn support_reverse_cont(
        &mut self,
    ) -> Option<gdbstub::target::ext::base::reverse_exec::ReverseContOps<'_, (), Self>> {
        None
    }
}

impl SingleThreadSingleStep for Emulator {
    fn step(&mut self, _signal: Option<gdbstub::common::Signal>) -> Result<(), Self::Error> {
        todo!()
    }
}


fn wait_for_gdb_connection(port: u16) -> io::Result<TcpStream> {
    let sockaddr = format!("localhost:{}", port);
    eprintln!("Waiting for a GDB connection on {:?}...", sockaddr);
    let sock = TcpListener::bind(sockaddr)?;
    let (stream, addr) = sock.accept()?;

    // Blocks until a GDB client connects via TCP.
    // i.e: Running `target remote localhost:<port>` from the GDB prompt.

    eprintln!("Debugger connected from {}", addr);
    Ok(stream) // `TcpStream` implements `gdbstub::Connection`
}

enum EventLoop {}

impl BlockingEventLoop for EventLoop {
    type Target = Emulator;
    type Connection = TcpStream;
    type StopReason = SingleThreadStopReason<u32>;

    fn wait_for_stop_reason(
        target: &mut Self::Target,
        conn: &mut Self::Connection,
    ) -> Result<
        Event<Self::StopReason>,
        WaitForStopReasonError<
            <Self::Target as Target>::Error,
            <Self::Connection as Connection>::Error,
        >,
    > {
        // let hit_bp = target.0.lock().unwrap().as_gga().options.running;
        // match () {
        //     _ if hit_bp => Ok(Event::TargetStopped(SingleThreadStopReason::SwBreak(()))),
        //     _ if target.1 => {
        //         target.1 = false;
        //         Ok(Event::TargetStopped(SingleThreadStopReason::DoneStep))
        //     }
        //     _ => Ok(Event::IncomingData(
        //         conn.read().map_err(WaitForStopReasonError::Connection)?,
        //     )),
        // }
        Ok(Event::TargetStopped(SingleThreadStopReason::DoneStep))
    }

    fn on_interrupt(
        _target: &mut Self::Target,
    ) -> Result<Option<Self::StopReason>, <Self::Target as Target>::Error> {
        // TODO handle this in the GUI
        Ok(Some(SingleThreadStopReason::Signal(Signal::SIGINT)))
    }
}

impl Emulator {
    pub fn init_debug(mut self){
        let stream = wait_for_gdb_connection(17633).unwrap();
        let debugger = GdbStub::new(stream);
        match debugger.run_blocking::<EventLoop>(&mut self) {
            Ok(_) => println!("Debugger disconnected!"),
            Err(e) => println!("gdbstub encountered an error: {}", e),
        }
    }
}