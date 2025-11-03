

/* Fully featured memory.x file */
MEMORY
{
  L2_LIM : ORIGIN = 0x80000000, LENGTH = 1M /* different RAM region for stack */
  RAM : ORIGIN = ORIGIN(L2_LIM) + LENGTH(L2_LIM), LENGTH = 63*1024K
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", L2_LIM);

/* _stext = ORIGIN(REGION_TEXT) + 4M;        Skip first 4M of text region */
_heap_size = 4M;                                /* Set heap size to 1KB */
_max_hart_id = 0;                               /* Two harts present */
_hart_stack_size = 16K;                          /* Set stack size per hart to 1KB */
_stack_start = ORIGIN(L2_LIM) + LENGTH(L2_LIM);

