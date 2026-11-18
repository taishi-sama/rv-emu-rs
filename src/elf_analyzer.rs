//ELF parsing and loading. The original version of this file came from
//https://github.com/takahirox/riscv-rust/blob/master/src/elf_analyzer.rs (see OTHER_LICENSES).
//The analyzer is the single parsing stack: every read is bounds-checked, so a truncated or
//malformed file surfaces as an error message instead of a panic, and both the PT_LOAD loader
//and the planned debugger/disassembler sit on the same API.

use std::collections::HashMap;

use crate::bus::{MemoryBus, RAM_ADDRESS, RAM_ADDRESS_END};

//ELF32 header and program-header constants.
const ELFCLASS32: u8 = 1;
const ELFDATA2LSB: u8 = 1;
const EM_RISCV: u16 = 243;
const PT_LOAD: u32 = 1;

/// ELF header
pub struct Header {
    pub e_width: u8, // 32 or 64
    pub e_class: u8,
    pub e_endian: u8,
    _e_elf_version: u8,
    _e_osabi: u8,
    _e_abi_version: u8,
    _e_type: u16,
    pub e_machine: u16,
    _e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    e_shoff: u64,
    _e_flags: u32,
    _e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    _e_shentsize: u16,
    e_shnum: u16,
    _e_shstrndx: u16,
}

/// ELF program header
pub struct ProgramHeader {
    pub p_type: u32,
    _p_flags: u32,
    pub p_offset: u64,
    _p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    _p_align: u64,
}

/// ELF section header
pub struct SectionHeader {
    sh_name: u32,
    pub sh_type: u32,
    _sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    _sh_link: u32,
    _sh_info: u32,
    _sh_addralign: u64,
    _sh_entsize: u64,
}

/// ELF symbol table entry
pub struct SymbolEntry {
    st_name: u32,
    st_info: u8,
    _st_other: u8,
    _st_shndx: u16,
    st_value: u64,
    _st_size: u64,
}

/// ELF file analyzer
pub struct ElfAnalyzer {
    data: Vec<u8>,
}

impl ElfAnalyzer {
    /// Creates a new `ElfAnalyzer`.
    ///
    /// # Arguments
    /// * `data` ELF file content binary
    pub fn new(data: Vec<u8>) -> Self {
        ElfAnalyzer { data: data }
    }

    /// Checks if ELF file content is valid
    // @TODO: Validate more precisely
    pub fn validate(&self) -> bool {
        // check ELF magic number
        if self.data.len() < 4
            || self.data[0] != 0x7f
            || self.data[1] != 0x45
            || self.data[2] != 0x4c
            || self.data[3] != 0x46
        {
            return false;
        }
        true
    }

    /// Reads ELF header
    pub fn read_header(&self) -> Result<Header, String> {
        let e_class = self.read_byte(4)?;

        let e_width = match e_class {
            1 => 32,
            2 => 64,
            _ => return Err(format!("unknown ELF class {e_class}")),
        };

        let e_endian = self.read_byte(5)?;
        let e_elf_version = self.read_byte(6)?;
        let e_osabi = self.read_byte(7)?;
        let e_abi_version = self.read_byte(8)?;

        let mut offset = 0x10;

        let e_type = self.read_halfword(offset)?;
        offset += 2;

        let e_machine = self.read_halfword(offset)?;
        offset += 2;

        let e_version = self.read_word(offset)?;
        offset += 4;

        let e_entry = match e_width {
            64 => {
                let data = self.read_doubleword(offset)?;
                offset += 8;
                data
            }
            _ => {
                let data = self.read_word(offset)?;
                offset += 4;
                data as u64
            }
        };

        let e_phoff = match e_width {
            64 => {
                let data = self.read_doubleword(offset)?;
                offset += 8;
                data
            }
            _ => {
                let data = self.read_word(offset)?;
                offset += 4;
                data as u64
            }
        };

        let e_shoff = match e_width {
            64 => {
                let data = self.read_doubleword(offset)?;
                offset += 8;
                data
            }
            _ => {
                let data = self.read_word(offset)?;
                offset += 4;
                data as u64
            }
        };

        let e_flags = self.read_word(offset)?;
        offset += 4;

        let e_ehsize = self.read_halfword(offset)?;
        offset += 2;

        let e_phentsize = self.read_halfword(offset)?;
        offset += 2;

        let e_phnum = self.read_halfword(offset)?;
        offset += 2;

        let e_shentsize = self.read_halfword(offset)?;
        offset += 2;

        let e_shnum = self.read_halfword(offset)?;
        offset += 2;

        let e_shstrndx = self.read_halfword(offset)?;
        //offset += 2;

        /*
        println!("ELF:{}", e_width);
        println!("e_endian:{:X}", e_endian);
        println!("e_elf_version:{:X}", e_elf_version);
        println!("e_osabi:{:X}", e_osabi);
        println!("e_abi_version:{:X}", e_abi_version);
        println!("e_type:{:X}", e_type);
        println!("e_machine:{:X}", e_machine);
        println!("e_version:{:X}", e_version);
        println!("e_entry:{:X}", e_entry);
        println!("e_phoff:{:X}", e_phoff);
        println!("e_shoff:{:X}", e_shoff);
        println!("e_flags:{:X}", e_flags);
        println!("e_ehsize:{:X}", e_ehsize);
        println!("e_phentsize:{:X}", e_phentsize);
        println!("e_phnum:{:X}", e_phnum);
        println!("e_shentsize:{:X}", e_shentsize);
        println!("e_shnum:{:X}", e_shnum);
        println!("e_shstrndx:{:X}", e_shstrndx);
        */

        Ok(Header {
            e_width: e_width,
            e_class: e_class,
            e_endian: e_endian,
            _e_elf_version: e_elf_version,
            _e_osabi: e_osabi,
            _e_abi_version: e_abi_version,
            _e_type: e_type,
            e_machine: e_machine,
            _e_version: e_version,
            e_entry: e_entry,
            e_phoff: e_phoff,
            e_shoff: e_shoff,
            _e_flags: e_flags,
            _e_ehsize: e_ehsize,
            e_phentsize: e_phentsize,
            e_phnum: e_phnum,
            _e_shentsize: e_shentsize,
            e_shnum: e_shnum,
            _e_shstrndx: e_shstrndx,
        })
    }

    /// Reads ELF program headers
    ///
    /// # Arguments
    /// * `header`
    pub fn read_program_headers(&self, header: &Header) -> Result<Vec<ProgramHeader>, String> {
        //The fields read below consume 32 bytes per ELF32 entry and 56 per ELF64 entry; the
        //gABI lets the table stride (e_phentsize) be larger, so entries are located by their
        //slot, not adjacent to the fields of the previous one.
        let entry_size = match header.e_width {
            64 => 56,
            32 => 32,
            _ => return Err(format!("unknown ELF width {}", header.e_width)),
        };
        if (header.e_phentsize as usize) < entry_size {
            return Err(format!(
                "unsupported e_phentsize {}, expected at least {entry_size}",
                header.e_phentsize
            ));
        }
        let mut headers = Vec::new();
        let mut offset = header.e_phoff as usize;
        for _i in 0..header.e_phnum {
            let entry_start = offset;
            let p_type = self.read_word(offset)?;
            offset += 4;

            let mut p_flags = 0;
            if header.e_width == 64 {
                p_flags = self.read_word(offset)?;
                offset += 4;
            }

            let p_offset = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let p_vaddr = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let p_paddr = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let p_filesz = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let p_memsz = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            if header.e_width == 32 {
                p_flags = self.read_word(offset)?;
                offset += 4;
            }

            //p_align is the last field of the entry; the next entry is located by its slot.
            let p_align = match header.e_width {
                64 => self.read_doubleword(offset)?,
                32 => self.read_word(offset)? as u64,
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            /*
            println!("");
            println!("Program:{:X}", i);
            println!("p_type:{:X}", p_type);
            println!("p_flags:{:X}", p_flags);
            println!("p_offset:{:X}", p_offset);
            println!("p_vaddr:{:X}", p_vaddr);
            println!("p_paddr:{:X}", p_paddr);
            println!("p_filesz:{:X}", p_filesz);
            println!("p_memsz:{:X}", p_memsz);
            println!("p_align:{:X}", p_align);
            println!("p_align:{:X}", p_align);
            */

            headers.push(ProgramHeader {
                p_type: p_type,
                _p_flags: p_flags,
                p_offset: p_offset,
                _p_vaddr: p_vaddr,
                p_paddr: p_paddr,
                p_filesz: p_filesz,
                p_memsz: p_memsz,
                _p_align: p_align,
            });
            //The next entry starts at its own slot, which may be padded past the fields read.
            offset = entry_start + header.e_phentsize as usize;
        }

        Ok(headers)
    }

    /// Reads ELF section headers
    ///
    /// # Arguments
    /// * `header`
    pub fn read_section_headers(&self, header: &Header) -> Result<Vec<SectionHeader>, String> {
        let mut headers = Vec::new();
        let mut offset = header.e_shoff as usize;
        for _i in 0..header.e_shnum {
            let sh_name = self.read_word(offset)?;
            offset += 4;

            let sh_type = self.read_word(offset)?;
            offset += 4;

            let sh_flags = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let sh_addr = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let sh_offset = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let sh_size = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let sh_link = self.read_word(offset)?;
            offset += 4;

            let sh_info = self.read_word(offset)?;
            offset += 4;

            let sh_addralign = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            let sh_entsize = match header.e_width {
                64 => {
                    let data = self.read_doubleword(offset)?;
                    offset += 8;
                    data
                }
                32 => {
                    let data = self.read_word(offset)?;
                    offset += 4;
                    data as u64
                }
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            /*
            println!("");
            println!("Section:{:X}", _i);
            println!("sh_name:{:X}", sh_name);
            println!("sh_type:{:X}", sh_type);
            println!("sh_flags:{:X}", sh_flags);
            println!("sh_addr:{:X}", sh_addr);
            println!("sh_offset:{:X}", sh_offset);
            println!("sh_size:{:X}", sh_size);
            println!("sh_link:{:X}", sh_link);
            println!("sh_info:{:X}", sh_info);
            println!("sh_addralign:{:X}", sh_addralign);
            println!("sh_entsize:{:X}", sh_entsize);
            */

            headers.push(SectionHeader {
                sh_name: sh_name,
                sh_type: sh_type,
                _sh_flags: sh_flags,
                sh_addr: sh_addr,
                sh_offset: sh_offset,
                sh_size: sh_size,
                _sh_link: sh_link,
                _sh_info: sh_info,
                _sh_addralign: sh_addralign,
                _sh_entsize: sh_entsize,
            });
        }

        Ok(headers)
    }

    /// Reads symbol entries of symbol table sections
    ///
    /// # Arguments
    /// * `Terminal`
    /// * `symbol_table_section_headers`
    pub fn read_symbol_entries(
        &self,
        header: &Header,
        symbol_table_section_headers: &Vec<&SectionHeader>,
    ) -> Result<Vec<SymbolEntry>, String> {
        let mut entries = Vec::new();
        for i in 0..symbol_table_section_headers.len() {
            let sh_offset = symbol_table_section_headers[i].sh_offset;
            let sh_size = symbol_table_section_headers[i].sh_size;

            let mut offset = sh_offset as usize;

            let entry_size = match header.e_width {
                64 => 24,
                32 => 16,
                _ => return Err(format!("unknown ELF width {}", header.e_width)),
            };

            for _j in 0..(sh_size / entry_size) {
                let st_name;
                let st_info;
                let _st_other;
                let _st_shndx;
                let st_value;
                let _st_size;

                match header.e_width {
                    64 => {
                        st_name = self.read_word(offset)?;
                        offset += 4;

                        st_info = self.read_byte(offset)?;
                        offset += 1;

                        _st_other = self.read_byte(offset)?;
                        offset += 1;

                        _st_shndx = self.read_halfword(offset)?;
                        offset += 2;

                        st_value = self.read_doubleword(offset)?;
                        offset += 8;

                        _st_size = self.read_doubleword(offset)?;
                        offset += 8;
                    }
                    32 => {
                        st_name = self.read_word(offset)?;
                        offset += 4;

                        st_value = self.read_word(offset)? as u64;
                        offset += 4;

                        _st_size = self.read_word(offset)? as u64;
                        offset += 4;

                        st_info = self.read_byte(offset)?;
                        offset += 1;

                        _st_other = self.read_byte(offset)?;
                        offset += 1;

                        _st_shndx = self.read_halfword(offset)?;
                        offset += 2;
                    }
                    _ => return Err(format!("unknown ELF width {}", header.e_width)),
                };

                /*
                println!("Symbol: {}", _j);
                println!("st_name: {:X}", st_name);
                println!("st_info: {:X}", st_info);
                println!("st_other: {:X}", _st_other);
                println!("st_shndx: {:X}", _st_shndx);
                println!("st_value: {:X}", st_value);
                println!("st_size: {:X}", _st_size);
                println!("");
                */

                entries.push(SymbolEntry {
                    st_name: st_name,
                    st_info: st_info,
                    _st_other: _st_other,
                    _st_shndx: _st_shndx,
                    st_value: st_value,
                    _st_size: _st_size,
                });
            }
        }
        Ok(entries)
    }

    /// Reads strings from a string table section
    ///
    /// # Arguments
    /// * `section_header` The header of the string table section
    /// * `index` Offset in the string table section
    fn read_strings(&self, section_header: &SectionHeader, index: u64) -> String {
        let sh_offset = section_header.sh_offset;
        let sh_size = section_header.sh_size;
        //Section-header fields are untrusted; keep the bounds math from wrapping on crafted
        //values so an overflowing lookup just ends the string.
        let table_end = sh_offset.saturating_add(sh_size);
        let mut pos = 0;
        let mut symbol = String::new();
        while let Some(addr) = sh_offset
            .checked_add(index)
            .and_then(|addr| addr.checked_add(pos))
        {
            if addr >= table_end {
                break;
            }
            //A truncated string table simply ends the string.
            let value = match self.read_byte(addr as usize) {
                Ok(value) => value,
                Err(_) => break,
            };
            if value == 0 {
                break;
            }
            symbol.push(value as char);
            pos += 1;
        }
        symbol
    }

    /// Creates a symbol - virtual address mapping from symbol entries
    /// and a string table section.
    ///
    /// # Arguments
    /// * `entries` Symbol entries
    /// * `string_table_section_header` The header of the string table section
    pub fn create_symbol_map(
        &self,
        entries: &Vec<SymbolEntry>,
        string_table_section_header: &SectionHeader,
    ) -> HashMap<String, u64> {
        let mut map = HashMap::default();
        for i in 0..entries.len() {
            let st_info = entries[i].st_info;
            let st_name = entries[i].st_name;
            let st_value = entries[i].st_value;

            // Stores only function and notype symbol
            if (st_info & 0x2) != 0x2 && (st_info & 0xf) != 0 {
                continue;
            }

            let symbol = self.read_strings(&string_table_section_header, st_name as u64);

            if !symbol.is_empty() {
                //println!("{} {:0x}", symbol, st_value);
                map.insert(symbol, st_value);
            }
        }
        map
    }

    /// Finds a program data section whose name is .tohost. If found this method
    /// returns an address of the section.
    ///
    /// # Arguments
    /// * `program_data_section_headers`
    /// * `string_table_section_headers`
    pub fn find_tohost_addr(
        &self,
        program_data_section_headers: &Vec<&SectionHeader>,
        string_table_section_headers: &Vec<&SectionHeader>,
    ) -> Option<u64> {
        let tohost_values = vec![0x2e, 0x74, 0x6f, 0x68, 0x6f, 0x73, 0x74, 0x00]; // ".tohost\null"
        for i in 0..program_data_section_headers.len() {
            let sh_addr = program_data_section_headers[i].sh_addr;
            let sh_name = program_data_section_headers[i].sh_name as u64;
            // Find all string sections so far.
            // @TODO: Is there a way to know which string table section
            //        sh_name of program data section points to?
            for j in 0..string_table_section_headers.len() {
                let sh_offset = string_table_section_headers[j].sh_offset;
                let sh_size = string_table_section_headers[j].sh_size;
                //Section-header fields are untrusted; saturating math makes an overflowing
                //lookup fail to match instead of wrapping.
                let table_end = sh_offset.saturating_add(sh_size);
                let name_start = sh_offset.saturating_add(sh_name);
                let mut found = true;
                for k in 0..tohost_values.len() as u64 {
                    let addr = name_start.saturating_add(k);
                    if addr >= table_end {
                        found = false;
                        break;
                    }
                    let byte = match self.read_byte(addr as usize) {
                        Ok(byte) => byte,
                        Err(_) => {
                            found = false;
                            break;
                        }
                    };
                    if byte != tohost_values[k as usize] {
                        found = false;
                        break;
                    }
                }
                if found {
                    return Some(sh_addr);
                }
            }
        }
        None
    }

    /// Reads a byte from ELF file content
    ///
    /// # Arguments
    /// * `offset`
    pub fn read_byte(&self, offset: usize) -> Result<u8, String> {
        self.data
            .get(offset)
            .copied()
            .ok_or_else(|| format!("truncated ELF: byte {offset} is out of range"))
    }

    /// Reads two bytes from ELF file content
    ///
    /// # Arguments
    /// * `offset`
    fn read_halfword(&self, offset: usize) -> Result<u16, String> {
        let mut data = 0;
        for i in 0..2 {
            data |= (self.read_byte(offset + i)? as u16) << (8 * i);
        }
        Ok(data)
    }

    /// Reads four bytes from ELF file content
    ///
    /// # Arguments
    /// * `offset`
    fn read_word(&self, offset: usize) -> Result<u32, String> {
        let mut data = 0;
        for i in 0..4 {
            data |= (self.read_byte(offset + i)? as u32) << (8 * i);
        }
        Ok(data)
    }

    /// Reads eight bytes from ELF file content
    ///
    /// # Arguments
    /// * `offset`
    fn read_doubleword(&self, offset: usize) -> Result<u64, String> {
        let mut data = 0;
        for i in 0..8 {
            data |= (self.read_byte(offset + i)? as u64) << (8 * i);
        }
        Ok(data)
    }
}

/// Loads every PT_LOAD segment of a little-endian ELF32 RISC-V executable into RAM and
/// returns the entry point.
///
/// Segments are placed at their physical address (`p_paddr`), which the bare-metal toolchains
/// used here keep equal to `p_vaddr`. The memory image beyond `p_filesz` is zeroed explicitly,
/// so a `.bss` overlapping an earlier segment still starts out zero.
pub fn elf_setup_bus(elf: Vec<u8>, bus: &mut MemoryBus) -> Result<u32, String> {
    let file_len = elf.len();
    let analyzer = ElfAnalyzer::new(elf);
    if !analyzer.validate() {
        return Err("not an ELF file".to_string());
    }
    let header = analyzer.read_header()?;
    if header.e_class != ELFCLASS32 {
        return Err(format!("not a 32-bit ELF (class {})", header.e_class));
    }
    if header.e_endian != ELFDATA2LSB {
        return Err("big-endian ELF files are not supported".to_string());
    }
    if header.e_machine != EM_RISCV {
        return Err(format!(
            "not a RISC-V ELF (e_machine {}, expected {EM_RISCV})",
            header.e_machine
        ));
    }
    if header.e_phnum == 0 {
        return Err("the ELF has no program headers to load".to_string());
    }
    let segments = analyzer.read_program_headers(&header)?;
    for (index, segment) in segments.iter().enumerate() {
        if segment.p_type != PT_LOAD {
            continue;
        }
        let offset = segment.p_offset as usize;
        let paddr = segment.p_paddr as u32;
        let filesz = segment.p_filesz as usize;
        let memsz = segment.p_memsz as u32;
        if filesz > memsz as usize {
            return Err(format!(
                "PT_LOAD {index} has p_filesz {filesz} larger than p_memsz {memsz}"
            ));
        }
        let Some(end) = offset.checked_add(filesz) else {
            return Err(format!(
                "PT_LOAD {index} file range {offset}..{filesz} overflows a 32-bit address"
            ));
        };
        if end > file_len {
            return Err(format!(
                "PT_LOAD {index} is truncated: {filesz} byte(s) at file offset {offset} exceed the file size {file_len}"
            ));
        }
        if paddr < RAM_ADDRESS || paddr as u64 + memsz as u64 > RAM_ADDRESS_END as u64 + 1 {
            return Err(format!(
                "PT_LOAD {index} at 0x{paddr:08x}..0x{:08x} does not fit in RAM",
                paddr as u64 + memsz as u64
            ));
        }
        for i in 0..filesz {
            let byte = analyzer.read_byte(offset + i)?;
            if !bus.write_raw_to_ram(paddr + i as u32, byte) {
                return Err(format!("PT_LOAD {index} does not fit in RAM"));
            }
        }
        //The memory image of a segment extends past its file image with zeroes; write them
        //explicitly so an overlapping earlier segment cannot leave data behind.
        for i in filesz..memsz as usize {
            if !bus.write_raw_to_ram(paddr + i as u32, 0) {
                return Err(format!("PT_LOAD {index} bss does not fit in RAM"));
            }
        }
    }
    //The class check above guarantees a 32-bit entry point.
    let entry = header.e_entry as u32;
    if !(RAM_ADDRESS..=RAM_ADDRESS_END).contains(&entry) {
        return Err(format!("entry point 0x{entry:08x} is outside RAM"));
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_halfword(bytes: &mut [u8], offset: usize, halfword: u16) {
        bytes[offset..offset + 2].copy_from_slice(&halfword.to_le_bytes());
    }

    fn put_word(bytes: &mut [u8], offset: usize, word: u32) {
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    //A minimal ELF32 with two PT_LOAD segments of one word each. The program header table
    //starts at byte 52 and strides by `phentsize`, so the second entry lives in its own slot
    //with padding before it when phentsize exceeds the 32 field bytes.
    fn elf32_with_phentsize(phentsize: u16) -> Vec<u8> {
        let data_offset = 52 + 2 * phentsize as usize;
        let mut elf = vec![0u8; data_offset + 8];
        elf[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        elf[4] = 1; //ELFCLASS32
        elf[5] = 1; //ELFDATA2LSB
        elf[6] = 1; //EV_CURRENT
        put_halfword(&mut elf, 16, 2); //ET_EXEC
        put_halfword(&mut elf, 18, 243); //EM_RISCV
        put_word(&mut elf, 24, 0x8000_0000); //e_entry
        put_word(&mut elf, 28, 52); //e_phoff
        put_halfword(&mut elf, 40, 52); //e_ehsize
        put_halfword(&mut elf, 42, phentsize);
        put_halfword(&mut elf, 44, 2); //e_phnum

        let second = 52 + phentsize as usize;
        put_word(&mut elf, 52, 1); //p_type = PT_LOAD
        put_word(&mut elf, 52 + 4, data_offset as u32);
        put_word(&mut elf, 52 + 12, 0x8000_0000); //p_paddr
        put_word(&mut elf, 52 + 16, 4); //p_filesz
        put_word(&mut elf, 52 + 20, 4); //p_memsz
        put_word(&mut elf, second, 1);
        put_word(&mut elf, second + 4, (data_offset + 4) as u32);
        put_word(&mut elf, second + 12, 0x8000_0100);
        put_word(&mut elf, second + 16, 4);
        put_word(&mut elf, second + 20, 4);
        put_word(&mut elf, data_offset, 0x1122_3344);
        put_word(&mut elf, data_offset + 4, 0x5566_7788);
        elf
    }

    //The gABI lets program headers sit e_phentsize bytes apart with padding between them; the
    //loader must find the second entry at its slot, not adjacent to the fields of the first.
    #[test]
    fn padded_program_header_entries_are_still_loaded() {
        let (mut bus, _audio) = MemoryBus::new();
        elf_setup_bus(elf32_with_phentsize(40), &mut bus).unwrap();
        assert_eq!(bus.read_word(0x8000_0000).unwrap(), 0x1122_3344);
        assert_eq!(bus.read_word(0x8000_0100).unwrap(), 0x5566_7788);
    }

    //A stride smaller than the field bytes cannot hold an entry and must be rejected.
    #[test]
    fn an_undersized_phentsize_is_rejected() {
        let (mut bus, _audio) = MemoryBus::new();
        let error = elf_setup_bus(elf32_with_phentsize(24), &mut bus).unwrap_err();
        assert!(error.contains("e_phentsize 24"), "got: {error}");
    }
}
