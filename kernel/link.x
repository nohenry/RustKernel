MEMORY
{
  RAM : ORIGIN = 0x20000000000, LENGTH = 8000000000K
}

SECTIONS
{
  .text :
  {
    *(.text .text.*);
  } > RAM
}