MEMORY
{
  RAM (RWXA): ORIGIN = 0x80000000, LENGTH = 64*1024K
}
ENTRY(__start)

SECTIONS
{
  . = ORIGIN(RAM);
  .text : { *(.text) }
  .data : { *(.data) }
  .bss : { *(.bss) }
  . =  ORIGIN(RAM) + LENGTH(RAM) - 0x4000;
  .stack : {*(.stack)}
}
