# ARM64 Memory

## Physical memory

- Physical-address width: 48 bits
- Frame size: 4 KiB
- Memory regions come from the device tree `/memory` node (usable) and `/reserved-memory` (reserved); regions reported by QEMU as `ram` are usable, everything else is reserved
- Kernel memory, bootloader memory, and device memory are excluded by the boot-time parser before the PMM sees the map
- No DMA limitations for the initial port (single device, coherent accesses not required)
- No direct physical-memory map is assumed; page-table frames are mapped through a fixed kernel virtual region chosen at boot

## Alignment

- All regions are normalized to 4 KiB frame boundaries; partial trailing frames are dropped
- Page tables are 4 KiB aligned and naturally aligned to their size

## Cacheability attributes

- Normal memory: Inner/Outer Write-Back, Read-Allocate, Write-Allocate, Inner Shareable
- Device memory: Device-nGnRnE (non-shareable, not gatherable, not reorderable, no early write acknowledgment)
- The `Attr` and memory-attribute fields of the page descriptors select the type; `MAIR_EL1` is programmed once at boot

## PMM integration

- The ARM64 `pmm` module converts the device-tree memory map into the generic `MemoryRegion` list the generic PMM consumes
- `init_from_memory_map` (generic) runs on the converted regions
- Reserved regions are never handed out; frame release is tracked; double release is rejected

## Documented concerns

- Physical-address width: 48 bits
- Memory-region alignment: 4 KiB frames
- Device-memory regions: PL011 (`0x09000000`), GICv2 (`0x08000000`/`0x08010000`), generic timer registers (system registers, not MMIO)
- Firmware-reserved areas: `/reserved-memory` from the device tree
- DMA limitations: none documented for the initial port
- Direct physical-memory map: not available; kernel virtual address is a fixed offset chosen at boot
