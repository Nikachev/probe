/* Memory layout for nice!nano v2 (nRF52840, 1 MB flash / 256 KB RAM)
 * running under the Adafruit nRF52 UF2 bootloader WITHOUT SoftDevice.
 *
 * Flash map:
 *   0x00000000 .. 0x00001000  MBR (Master Boot Record)
 *   0x00001000 .. 0x000F4000  Application  <-- we live here
 *   0x000F4000 .. 0x00100000  Bootloader + MBR params + settings (do NOT touch)
 *
 * The MBR forwards interrupts to the application at 0x1000 via
 * SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET. VTOR is set to 0x1000 at
 * startup so exceptions/interrupts vector into our table.
 *
 * BOOT_VECTORS at 0x0 is needed by target test binaries (target_blinky,
 * target_fault, target_rtt) which are flashed via SWD onto Board B.
 * Board B has no MBR/bootloader, so the Cortex-M hardware reset reads
 * SP and Reset_Handler from address 0x0.  The main app (flashed via
 * UF2) does NOT use this section -- it is ignored by the UF2 bootloader
 * since only data starting at BASE (0x1000) is written to flash.
 */
MEMORY
{
  BOOT_VECTORS : ORIGIN = 0x00000000, LENGTH = 0x100
  FLASH        : ORIGIN = 0x00001000, LENGTH = 0xF3000  /* 0xF4000 - 0x1000 = 972 KB */
  RAM          : ORIGIN = 0x20000000, LENGTH = 256K
}

SECTIONS
{
  .boot_vectors :
  {
    KEEP(*(.boot_vectors));
  } > BOOT_VECTORS
}
