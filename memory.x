/* Memory layout for nice!nano v2 (nRF52840, 1 MB flash / 256 KB RAM)
 * running under the Adafruit nRF52 UF2 bootloader WITH the S140 6.1.1 SoftDevice
 * (the stock nice!nano configuration, confirmed via INFO_UF2.TXT).
 *
 * Flash map:
 *   0x00000000 .. 0x00001000  MBR (Master Boot Record)
 *   0x00001000 .. 0x00026000  SoftDevice S140 6.1.1 (present, NOT enabled by us)
 *   0x00026000 .. 0x000F4000  Application  <-- we live here
 *   0x000F4000 .. 0x00100000  Bootloader + MBR params + settings (do NOT touch)
 *
 * We do not use the SoftDevice (bare-metal CMSIS-DAP probe), but we must not
 * overwrite it, because the bootloader's app-start logic relies on it being
 * present and jumps to the application at 0x26000. VTOR is set to 0x26000 at
 * startup so exceptions/interrupts vector into our table.
 */
MEMORY
{
  FLASH : ORIGIN = 0x00026000, LENGTH = 0xCE000  /* 0xF4000 - 0x26000 = 824 KB */
  RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
