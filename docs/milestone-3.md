# Milestone 3: Physical Memory Management

## Objective

Establish a safe physical-frame allocator built from the bootloader's memory map. This allocator will be the foundation for page tables, the kernel heap, and future subsystems.

## Scope

- Memory-map parsing and normalization from `bootloader_api::info::MemoryRegions`.
- Classification of memory regions into allocatable and reserved.
- Definition of a physical-frame abstraction (4 KiB).
- A physical-frame allocator (bitmap is the target design; bump is acceptable for initial validation if needed).
- Ownership rules and safety invariants.
- Integration into the kernel with diagnostics.
- Unit tests for allocator logic.
- Ensure the existing boot test still passes.

## Memory-map source

The kernel receives `BootInfo` from the `bootloader` crate (bootloader 0.11.x, UEFI). We treat the memory map as external input and validate it before using it. The allocator must only allocate from regions explicitly marked as usable by the bootloader.

## Memory-region rules

Usable memory: allocate from regions with `MemoryRegionKind::Usable`. Never allocate from `Reserved`, `Bootloader`, `Framebuffer`, `Unknown`, `Kernel`, or any other kind unless explicitly justified and documented. Unknown regions are treated as unusable. Framebuffer memory remains reserved until its lifetime is explicitly tracked.

## Physical-frame model

- Frame size = 4 KiB (the architecture base page size for x86_64). No huge pages in this milestone.
- A frame is identified by its physical start address, aligned to 4 KiB.
- Returned frames must lie entirely within a usable region from the bootloader.
- All addresses used for frame arithmetic must be checked to avoid overflow.

## Allocator design

- Target: bitmap allocator (supports allocation and future free operations). Simpler bump allocator may be used as an interim step for validation, but the milestone goal is a bitmap-based design or at least one that can be extended to support release.
- The allocator should normalize usable regions (align up start if needed, align down end, drop partial trailing bytes, merge as appropriate). Do not silently merge non-contiguous regions.
- Bitmap storage must be placed in a safe location; if the bitmap is allocated from physical memory, the region used by the bitmap itself must be marked as used/reserved.
- Allocation: return a frame address or an error if none available. Do not return partial frames.
- Deallocation (if implemented): detect double-free, detect frames outside managed ranges, and reject invalid frees. At minimum, document behavior even if not fully implemented yet.

## Ownership and lifetime

- The allocator is a kernel-global resource. Initially single-core/single-threaded.
- Once initialized, it owns the set of physical frames it manages.
- The kernel image, stack, bootloader data, framebuffer, and bitmap area are excluded from allocation.
- Allocation can occur before/at early boot (before interrupts). No locking required in the first phase; document when synchronization becomes necessary.
- Exhaustion: define a clear error path (panic or return error) and document it.

## Safety invariants

1. Every allocated frame is 4 KiB aligned.
2. Every allocated frame is within a bootloader-marked usable region.
3. No frame is allocated twice.
4. The allocator never hands out memory that overlaps kernel/bootloader/firmware-reserved structures.
5. Region arithmetic is checked (no integer overflow).
6. Unknown memory is never treated as free.
7. Partial frames are never returned.

## Testing strategy

Pure unit tests (no QEMU/hardware) for:
- Empty/invalid maps
- Single/multiple usable regions
- Unaligned boundaries
- Regions < 4 KiB
- Adjacent regions
- Reserved regions between usable regions
- Exhaustion
- Allocation/release (if supported)
- Double free, invalid free, invalid addresses
- Overflow cases
- Large physical addresses

## Diagnostics

- On initialization, log: count of usable regions, total usable bytes/frames, first/last managed ranges, largest usable region, and bitmap size/location if relevant.
- Allocation failures should produce clear diagnostics.

## Out of scope

- Virtual memory redesign, page tables, heap allocator, user processes, scheduling, SMP, DMA, filesystems, drivers, ARM64, huge pages, memory compression, swapping, NUMA.

## Exit criteria

- [ ] Usable memory regions parsed safely and normalized.
- [ ] Reserved regions never allocated.
- [ ] Physical frames are 4 KiB aligned; partial frames never returned.
- [ ] Allocation works for valid cases.
- [ ] Allocation exhaustion handled with defined behavior.
- [ ] Frame ownership rules documented.
- [ ] Safety invariants documented.
- [ ] Pure allocator tests pass.
- [ ] QEMU boot reports allocator initialization/summary.
- [ ] Kernel still passes the complete boot test (`cargo test -p kernel --test boot`).
- [ ] No new subsystem depends on undocumented allocator behavior.
