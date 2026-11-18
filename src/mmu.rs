use crate::{
    bus::{AtomicAccess, MemoryBus},
    cpu::{MSTATUS_MPP_SHIFT, MSTATUS_MPRV, MSTATUS_MXR, MSTATUS_SUM, PrivilegeMode},
    plic::PlicContext,
    traps::{Trap, TrapType},
};

const PAGE_SHIFT: u32 = 12;
const PAGE_SIZE: u32 = 1 << PAGE_SHIFT;
const PAGE_MASK: u32 = PAGE_SIZE - 1;

const PTE_V: u32 = 1 << 0;
const PTE_R: u32 = 1 << 1;
const PTE_W: u32 = 1 << 2;
const PTE_X: u32 = 1 << 3;
const PTE_U: u32 = 1 << 4;
const PTE_A: u32 = 1 << 6;
const PTE_D: u32 = 1 << 7;

//Sv32 physical addresses are 34 bits, but the bus is 32 bits wide. Page table entries whose
//PPN would address above 4 GiB raise an access fault, as if the bus had rejected the access.
const MAX_SUPPORTED_PPN: u32 = 1 << 20;

const TLB_ENTRIES: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AccessType {
    Fetch,
    Load,
    Store,
}

impl AccessType {
    fn access_fault(self) -> TrapType {
        match self {
            AccessType::Fetch => TrapType::InstructionAccessFault,
            AccessType::Load => TrapType::LoadAccessFault,
            AccessType::Store => TrapType::StoreAccessFault,
        }
    }
    fn page_fault(self) -> TrapType {
        match self {
            AccessType::Fetch => TrapType::InstructionPageFault,
            AccessType::Load => TrapType::LoadPageFault,
            AccessType::Store => TrapType::StorePageFault,
        }
    }
}

// Direct TLB buffer entry.
#[derive(Clone, Copy, Default)]
struct TlbEntry {
    /// vpn[1] and vpn[0] of virtual address
    vpn: u32,
    /// Translated page address
    pa_page: u32,
    /// leaf page table as itself
    pte: u32,
    /// Physical address of leaf entry to refresh flags
    pte_addr: u32,
    /// Address-space identifier
    asid_tag: u16,
}

pub struct MMU {
    pub bus: MemoryBus,
    satp: u32,
    privilege: PrivilegeMode,
    //Privilege mstatus.MPRV redirects data accesses to (the value of mstatus.MPP), or None when
    //MPRV is clear or MPP selects machine mode. Fetches are never redirected.
    mprv_privilege: Option<PrivilegeMode>,
    //Privilege data accesses are translated and protected at: the MPRV-selected mode while
    //MPRV redirects, else the current mode.
    data_privilege: PrivilegeMode,
    //Cached translation switches for the hot path: fetches are translated unless the hart is
    //in M-mode, loads and stores unless the MPRV-selected mode is M. Recomputed only when
    //satp, the privilege mode or mstatus change; translate() reads one bool per access
    //instead of rederiving the decision every time.
    fetch_translated: bool,
    data_translated: bool,
    sum: bool,
    mxr: bool,
    tlb: Vec<TlbEntry>,
    // Used to quickly invalidate cache
    current_asid_tag: u16,
}

// Sligltly optimize validity test by raising deliberately unused bit on every write, so comparisons with zero-filled buffer always fail even on ASID=0
const ASID_VALIDITY_MARKER: u16 = 0x8000;

impl MMU {
    pub fn new(bus: MemoryBus) -> Self {
        let mut mmu = MMU {
            bus,
            satp: 0,
            privilege: PrivilegeMode::Machine,
            mprv_privilege: None,
            data_privilege: PrivilegeMode::Machine,
            fetch_translated: false,
            data_translated: false,
            sum: false,
            mxr: false,
            tlb: vec![TlbEntry::default(); TLB_ENTRIES],
            current_asid_tag: ASID_VALIDITY_MARKER,
        };
        mmu.recompute_translation_switches();
        mmu
    }
    //satp,
    pub fn satp(&self) -> u32 {
        self.satp
    }
    pub fn set_satp(&mut self, satp: u32) {
        self.satp = satp;
        self.current_asid_tag = ((satp >> 22) & 0x1ff) as u16 | ASID_VALIDITY_MARKER;
        self.recompute_translation_switches();
    }
    //Privilege and the SUM/MXR bits of mstatus complete the translation context. The CPU updates
    //them only when they actually change.
    pub fn set_privilege(&mut self, privilege: PrivilegeMode) {
        self.privilege = privilege;
        self.recompute_translation_switches();
    }
    pub fn update_from_mstatus(&mut self, mstatus: u64) {
        self.sum = mstatus & MSTATUS_SUM as u64 != 0;
        self.mxr = mstatus & MSTATUS_MXR as u64 != 0;
        self.mprv_privilege = Self::mprv_privilege_from_mstatus(mstatus);
        self.recompute_translation_switches();
    }
    //Derives the cached translation switches. Every path that changes satp, the privilege
    //mode or mstatus funnels through set_satp/set_privilege/update_from_mstatus, so the
    //per-access checks in translate() only read the cached fields.
    fn recompute_translation_switches(&mut self) {
        let (data_privilege, fetch_translated, data_translated) =
            Self::translation_switches(self.satp, self.privilege, self.mprv_privilege);
        self.data_privilege = data_privilege;
        self.fetch_translated = fetch_translated;
        self.data_translated = data_translated;
    }
    //Loads and stores are translated and protected as if the hart ran in the MPRV-selected
    //mode; instruction fetch always uses the current mode. M-mode (and the never-entered
    //reserved mode) bypasses translation entirely. mmu-context-check re-derives these values
    //to prove the caches are never stale.
    fn translation_switches(
        satp: u32,
        privilege: PrivilegeMode,
        mprv_privilege: Option<PrivilegeMode>,
    ) -> (PrivilegeMode, bool, bool) {
        let data_privilege = mprv_privilege.unwrap_or(privilege);
        let paging = satp >> 31 != 0;
        (
            data_privilege,
            paging && !matches!(privilege, PrivilegeMode::Machine | PrivilegeMode::Reserved),
            paging
                && !matches!(
                    data_privilege,
                    PrivilegeMode::Machine | PrivilegeMode::Reserved
                ),
        )
    }
    //Privilege the effective-privilege redirect selects: MPP=U or S while MPRV is set; MPP=M
    //(and the coerced reserved encoding, which cannot be written here) selects machine mode and
    //thus no translation.
    fn mprv_privilege_from_mstatus(mstatus: u64) -> Option<PrivilegeMode> {
        if mstatus & MSTATUS_MPRV as u64 == 0 {
            return None;
        }
        match (mstatus >> MSTATUS_MPP_SHIFT) & 0b11 {
            0 => Some(PrivilegeMode::User),
            1 => Some(PrivilegeMode::Supervisor),
            _ => None,
        }
    }
    //Used by the mmu-context-check feature to catch a missed sync after a context change. It
    //verifies the CPU-owned inputs and the caches derived from them, so bypassing the setters
    //shows up here instead of leaving silently stale translation flags.
    #[cfg(feature = "mmu-context-check")]
    pub fn context_matches(&self, privilege: PrivilegeMode, mstatus: u64) -> bool {
        let mprv_privilege = Self::mprv_privilege_from_mstatus(mstatus);
        if self.privilege != privilege
            || self.sum != (mstatus & MSTATUS_SUM as u64 != 0)
            || self.mxr != (mstatus & MSTATUS_MXR as u64 != 0)
            || self.mprv_privilege != mprv_privilege
        {
            return false;
        }
        let (data_privilege, fetch_translated, data_translated) =
            Self::translation_switches(self.satp, privilege, mprv_privilege);
        self.data_privilege == data_privilege
            && self.fetch_translated == fetch_translated
            && self.data_translated == data_translated
    }
    pub fn flush_tlb(&mut self) {
        self.tlb.fill(TlbEntry::default());
    }
    #[inline]
    fn permits(&self, pte: u32, access: AccessType) -> bool {
        //The effective privilege follows the access type: fetches run in the current mode,
        //loads and stores in the MPRV-selected one. The SUM and MXR views of mstatus apply to
        //both, as the spec makes MPRV accesses "as though the current privilege mode were set
        //to MPP".
        let privilege = match access {
            AccessType::Fetch => self.privilege,
            AccessType::Load | AccessType::Store => self.data_privilege,
        };
        let readable = pte & PTE_R != 0 || (self.mxr && pte & PTE_X != 0);
        let writable = pte & PTE_W != 0;
        let executable = pte & PTE_X != 0;
        let user = pte & PTE_U != 0;
        match privilege {
            PrivilegeMode::Machine | PrivilegeMode::Reserved => true,
            PrivilegeMode::Supervisor => {
                if access == AccessType::Fetch {
                    //S-mode never executes from a user page, regardless of SUM.
                    executable && !user
                } else {
                    let allowed = match access {
                        AccessType::Load => readable,
                        AccessType::Store => writable,
                        AccessType::Fetch => unreachable!(),
                    };
                    allowed && (!user || self.sum)
                }
            }
            PrivilegeMode::User => match access {
                AccessType::Fetch => executable && user,
                AccessType::Load => readable && user,
                AccessType::Store => writable && user,
            },
        }
    }
    #[inline(always)]
    fn translate(&mut self, va: u32, access: AccessType) -> Result<u32, Trap> {
        let translates = match access {
            AccessType::Fetch => self.fetch_translated,
            AccessType::Load | AccessType::Store => self.data_translated,
        };
        if !translates {
            return Ok(va);
        }
        let vpn = va >> PAGE_SHIFT;
        let index = (vpn as usize) & (TLB_ENTRIES - 1);
        let entry = self.tlb[index];
        if entry.asid_tag == self.current_asid_tag && entry.vpn == vpn {
            if !self.permits(entry.pte, access) {
                return Err(Trap {
                    tcause: access.page_fault(),
                    tval: va,
                });
            }
            //A page that was read first already has A set, but D is only set by a store, which
            //may still miss the PTE even though its translation is cached.
            if access == AccessType::Store && entry.pte & PTE_D == 0 {
                let updated = entry.pte | PTE_A | PTE_D;
                self.bus
                    .write_word(entry.pte_addr, updated)
                    .map_err(|_| Trap {
                        tcause: TrapType::StoreAccessFault,
                        tval: va,
                    })?;
                self.tlb[index].pte = updated;
            }
            return Ok(entry.pa_page | (va & PAGE_MASK));
        }
        let (pte, pa_page, pte_addr) = self.walk(va, access)?;
        self.tlb[index] = TlbEntry {
            vpn,
            pa_page,
            pte,
            pte_addr,
            asid_tag: self.current_asid_tag,
        };
        Ok(pa_page | (va & PAGE_MASK))
    }
    fn get_ppn(pte: u32) -> u32 {
        (Self::get_ppn1(pte) << 10) | Self::get_ppn0(pte)
    }
    #[inline(always)]
    fn get_ppn1(pte: u32) -> u32 {
        pte >> 20
    }
    #[inline(always)]
    fn get_ppn0(pte: u32) -> u32 {
        (pte >> 10) & 0x3ff
    }
    #[inline(always)]
    fn get_vpn1(va: u32) -> u32 {
        (va >> 22) & 0x3ff
    }
    #[inline(always)]
    fn get_vpn0(va: u32) -> u32 {
        (va >> 12) & 0x3ff
    }
    //Walks Sv32 page tables. PTE reads are physical accesses; a bus error or a PPN that would
    //address above 4 GiB becomes an access fault of the original access type.
    #[inline(never)]
    fn walk(&mut self, va: u32, access: AccessType) -> Result<(u32, u32, u32), Trap> {
        let access_fault = Trap {
            tcause: access.access_fault(),
            tval: va,
        };
        let page_fault = Trap {
            tcause: access.page_fault(),
            tval: va,
        };
        let root_ppn = self.satp & 0x003f_ffff;
        if root_ppn >= MAX_SUPPORTED_PPN {
            return Err(access_fault);
        }
        let pte1_addr = (root_ppn << PAGE_SHIFT) | (Self::get_vpn1(va) << 2);
        let pte1 = self.bus.read_word(pte1_addr).map_err(|_| access_fault)?;
        if pte1 & PTE_V == 0 || pte1 & PTE_R == 0 && pte1 & PTE_W != 0 {
            return Err(page_fault);
        }
        let (pte, pte_addr, superpage) = if pte1 & (PTE_R | PTE_X) != 0 {
            (pte1, pte1_addr, true)
        } else {
            let ppn = Self::get_ppn(pte1);
            if ppn >= MAX_SUPPORTED_PPN {
                return Err(access_fault);
            }
            let pte0_addr = (ppn << PAGE_SHIFT) | (Self::get_vpn0(va) << 2);
            let pte0 = self.bus.read_word(pte0_addr).map_err(|_| access_fault)?;
            if pte0 & PTE_V == 0
                || pte0 & PTE_R == 0 && pte0 & PTE_W != 0
                || pte0 & (PTE_R | PTE_X) == 0
            {
                return Err(page_fault);
            }
            (pte0, pte0_addr, false)
        };
        //A superpage must be aligned: ppn[9:0] has to be zero.
        if superpage && Self::get_ppn0(pte) != 0 {
            return Err(page_fault);
        }
        if !self.permits(pte, access) {
            return Err(page_fault);
        }
        let ppn = Self::get_ppn(pte);
        if ppn >= MAX_SUPPORTED_PPN {
            return Err(access_fault);
        }
        // Emulator updates A(ccess)/D(irty) flags.
        // Theoretically could just delegated to the firmware.
        let needed = if access == AccessType::Store {
            PTE_D | PTE_A
        } else {
            PTE_A
        };
        let pte = if pte & needed != needed {
            let updated = pte | needed;
            self.bus
                .write_word(pte_addr, updated)
                .map_err(|_| access_fault)?;
            updated
        } else {
            pte
        };
        //A superpage maps a 4 MiB region, so virtual address bits [21:12] are part of the
        //physical address; the TLB caches the resulting 4 KiB page base.
        let pa_page = if superpage {
            (ppn << PAGE_SHIFT) | (va & 0x003f_f000)
        } else {
            ppn << PAGE_SHIFT
        };
        Ok((pte, pa_page, pte_addr))
    }
    fn bus_error(address: u32, cause: TrapType) -> Trap {
        Trap {
            tcause: cause,
            tval: address,
        }
    }
    //The core's view of the CLINT, which is a shared device on the bus.
    pub fn mtime(&mut self) -> u64 {
        self.bus.clint.mtime()
    }
    pub fn msip(&mut self) -> bool {
        self.bus.clint.msip()
    }
    pub fn mtip(&mut self) -> bool {
        self.bus.clint.mtip()
    }
    //Machine external interrupt line
    #[inline]
    pub fn meip(&mut self) -> bool {
        self.bus.external_interrupt_pending(PlicContext::Machine)
    }
    //Supervisor external interrupt line
    #[inline]
    pub fn seip(&mut self) -> bool {
        self.bus.external_interrupt_pending(PlicContext::Supervisor)
    }
    //Sample device interrupt lines into the PLIC before the CPU samples its external lines.
    #[inline]
    pub fn poll_irqs(&mut self) {
        self.bus.poll_irq_lines();
    }
    pub fn fetch_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        let pa = self.translate(address, AccessType::Fetch)?;
        self.bus
            .fetch_halfword(pa)
            .map_err(|_| Self::bus_error(address, TrapType::InstructionAccessFault))
    }
    pub fn read_word_atomically(
        &mut self,
        address: u32,
        access: AtomicAccess,
    ) -> Result<u32, Trap> {
        let access_type = match access {
            AtomicAccess::Load => AccessType::Load,
            AtomicAccess::Store => AccessType::Store,
        };
        let pa = self.translate(address, access_type)?;
        self.bus
            .read_word_atomically(pa, access)
            .map_err(|trap| Self::bus_error(address, trap.tcause))
    }
    pub fn write_word_atomically(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        let pa = self.translate(address, AccessType::Store)?;
        self.bus
            .write_word_atomically(pa, word)
            .map_err(|trap| Self::bus_error(address, trap.tcause))
    }
    pub fn read_word(&mut self, address: u32) -> Result<u32, Trap> {
        if !self.data_translated {
            return self.bus.read_word(address);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 4 {
            let pa = self.translate(address, AccessType::Load)?;
            return self
                .bus
                .read_word(pa)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        // If address is split on page boundary, read each byte separately,
        let mut value = 0;
        for i in 0..4 {
            let pa = self.translate(address.wrapping_add(i), AccessType::Load)?;
            let byte = self
                .bus
                .read_byte(pa)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i), trap.tcause))?;
            value |= (byte as u32) << (i * 8);
        }
        Ok(value)
    }
    //A 64-bit access is either two adjacent words within one page or a byte-wise sequence
    //across a page boundary. Translating every byte before the first bus access keeps a fault
    //from leaving a partially written store behind.
    pub fn read_dword(&mut self, address: u32) -> Result<u64, Trap> {
        if !self.data_translated {
            return self.bus.read_dword(address);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 8 {
            let pa = self.translate(address, AccessType::Load)?;
            return self
                .bus
                .read_dword(pa)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        let mut value = 0u64;
        for i in 0..8 {
            let pa = self.translate(address.wrapping_add(i), AccessType::Load)?;
            let byte = self
                .bus
                .read_byte(pa)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i), trap.tcause))?;
            value |= (byte as u64) << (i * 8);
        }
        Ok(value)
    }
    pub fn read_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        if !self.data_translated {
            return self.bus.read_halfword(address);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 2 {
            let pa = self.translate(address, AccessType::Load)?;
            return self
                .bus
                .read_halfword(pa)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        // If address is split on page boundary, read each byte separately,
        let mut value = 0u16;
        for i in 0..2 {
            let pa = self.translate(address.wrapping_add(i), AccessType::Load)?;
            let byte = self
                .bus
                .read_byte(pa)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i), trap.tcause))?;
            value |= (byte as u16) << (i * 8);
        }
        Ok(value)
    }
    pub fn read_byte(&mut self, address: u32) -> Result<u8, Trap> {
        let pa = self.translate(address, AccessType::Load)?;
        self.bus
            .read_byte(pa)
            .map_err(|trap| Self::bus_error(address, trap.tcause))
    }
    pub fn write_word(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        if !self.data_translated {
            return self.bus.write_word(address, word);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 4 {
            let pa = self.translate(address, AccessType::Store)?;
            return self
                .bus
                .write_word(pa, word)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        // If address is split on page boundary, read each byte separately,
        let mut physical = [0u32; 4];
        for (i, pa) in physical.iter_mut().enumerate() {
            *pa = self.translate(address.wrapping_add(i as u32), AccessType::Store)?;
        }
        for (i, pa) in physical.iter().enumerate() {
            self.bus
                .write_byte(*pa, (word >> (i * 8)) as u8)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i as u32), trap.tcause))?;
        }
        Ok(())
    }
    pub fn write_dword(&mut self, address: u32, value: u64) -> Result<(), Trap> {
        if !self.data_translated {
            return self.bus.write_dword(address, value);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 8 {
            let pa = self.translate(address, AccessType::Store)?;
            return self
                .bus
                .write_dword(pa, value)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        //Translate every byte before writing anything, so a page fault cannot leave a partial
        //store behind.
        let mut physical = [0u32; 8];
        for (i, pa) in physical.iter_mut().enumerate() {
            *pa = self.translate(address.wrapping_add(i as u32), AccessType::Store)?;
        }
        for (i, pa) in physical.iter().enumerate() {
            self.bus
                .write_byte(*pa, (value >> (i * 8)) as u8)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i as u32), trap.tcause))?;
        }
        Ok(())
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        if !self.data_translated {
            return self.bus.write_halfword(address, halfword);
        }
        if address & PAGE_MASK <= PAGE_SIZE - 2 {
            let pa = self.translate(address, AccessType::Store)?;
            return self
                .bus
                .write_halfword(pa, halfword)
                .map_err(|trap| Self::bus_error(address, trap.tcause));
        }
        // If address is split on page boundary, read each byte separately,
        let mut physical = [0u32; 2];
        for (i, pa) in physical.iter_mut().enumerate() {
            *pa = self.translate(address.wrapping_add(i as u32), AccessType::Store)?;
        }
        for (i, pa) in physical.iter().enumerate() {
            self.bus
                .write_byte(*pa, (halfword >> (i * 8)) as u8)
                .map_err(|trap| Self::bus_error(address.wrapping_add(i as u32), trap.tcause))?;
        }
        Ok(())
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        let pa = self.translate(address, AccessType::Store)?;
        self.bus
            .write_byte(pa, byte)
            .map_err(|trap| Self::bus_error(address, trap.tcause))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MemoryBus;

    //A page table mapping 0x00400000 -> 0x80000000 as a user page, with the root table at
    //0x80400000 and the leaf table at 0x80401000.
    fn mmu_with_user_page() -> MMU {
        let (mut bus, _audio) = MemoryBus::new();
        for (offset, byte) in 0x1234_5678u32.to_le_bytes().iter().enumerate() {
            assert!(bus.write_raw_to_ram(0x8000_0000 + offset as u32, *byte));
        }
        let level1 = 0x2010_0401u32; //pointer to the leaf table at 0x80401000
        let level0 = 0x2000_00d7u32; //R|W|U|V|A|D leaf mapping to 0x80000000
        for (address, word) in [(0x8040_0004u32, level1), (0x8040_1000u32, level0)] {
            for (offset, byte) in word.to_le_bytes().iter().enumerate() {
                assert!(bus.write_raw_to_ram(address + offset as u32, *byte));
            }
        }
        let mut mmu = MMU::new(bus);
        mmu.set_satp(0x8000_0000 | 0x8_0400);
        mmu.set_privilege(PrivilegeMode::Machine);
        mmu
    }

    //MPRV=1 with MPP=U: M-mode loads and stores are translated and protected as if the hart
    //ran in U-mode; without MPRV they are untranslated, and 0x00400000 is behind no device.
    #[test]
    fn mprv_redirects_m_mode_data_accesses() {
        let mut mmu = mmu_with_user_page();
        mmu.update_from_mstatus(MSTATUS_MPRV as u64);
        assert_eq!(mmu.read_word(0x0040_0000).unwrap(), 0x1234_5678);

        mmu.update_from_mstatus(0);
        assert_eq!(
            mmu.read_word(0x0040_0000).unwrap_err().tcause,
            TrapType::LoadAccessFault
        );
    }

    //The redirected access obeys the redirected mode's protection: the page is user-only, so
    //an MPP=S load needs SUM to reach it.
    #[test]
    fn mprv_uses_the_redirected_modes_permissions() {
        let mut mmu = mmu_with_user_page();
        mmu.update_from_mstatus((MSTATUS_MPRV | (1 << MSTATUS_MPP_SHIFT)) as u64);
        assert_eq!(
            mmu.read_word(0x0040_0000).unwrap_err().tcause,
            TrapType::LoadPageFault
        );

        mmu.update_from_mstatus((MSTATUS_MPRV | MSTATUS_SUM | (1 << MSTATUS_MPP_SHIFT)) as u64);
        assert_eq!(mmu.read_word(0x0040_0000).unwrap(), 0x1234_5678);
    }

    //MPRV never affects instruction fetches, which stay untranslated in M-mode.
    #[test]
    fn mprv_never_affects_fetches() {
        let mut mmu = mmu_with_user_page();
        mmu.update_from_mstatus(MSTATUS_MPRV as u64);
        assert_eq!(
            mmu.fetch_halfword(0x0040_0000).unwrap_err().tcause,
            TrapType::InstructionAccessFault
        );
    }

    //Translation also follows plain privilege changes, not just the MPRV redirect: leaving
    //M-mode with paging on turns translation on, coming back turns it off. This is the
    //recompute path every context switch rides on.
    #[test]
    fn privilege_changes_toggle_translation() {
        let mut mmu = mmu_with_user_page();
        //The user page is not readable from S-mode without SUM.
        mmu.set_privilege(PrivilegeMode::Supervisor);
        assert_eq!(
            mmu.read_word(0x0040_0000).unwrap_err().tcause,
            TrapType::LoadPageFault
        );
        //U-mode reads it through translation.
        mmu.set_privilege(PrivilegeMode::User);
        assert_eq!(mmu.read_word(0x0040_0000).unwrap(), 0x1234_5678);
        //Back in M-mode the access is untranslated and hits no device.
        mmu.set_privilege(PrivilegeMode::Machine);
        assert_eq!(
            mmu.read_word(0x0040_0000).unwrap_err().tcause,
            TrapType::LoadAccessFault
        );
    }
}
