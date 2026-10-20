# Milestone 5: Kernel Heap Allocator

## Objective

Provide the Elvaris kernel with a general-purpose dynamic memory allocator.

The kernel heap is the first consumer of the two subsystems built in the previous
milestones:

```text
Physical frames
      ↓
Virtual mappings
      ↓
Kernel heap region
      ↓
Heap allocator
      ↓
Kernel data structures
```

The heap gives later kernel subsystems (collections, device structures, process
metadata, filesystem structures, IPC objects, scheduler data, driver state) a
single, well-defined way to obtain and release memory.

The heap must not hide failures in the PMM or the virtual-memory subsystem. Every
page-allocation failure and every mapping failure must remain observable.

## Scope

- A reserved kernel heap virtual-address range, documented as a provisional
  layout.
- A heap allocator with allocation and deallocation, arbitrary power-of-two
  alignment, block splitting, and coalescing of adjacent free blocks.
- A heap front-end that maps the initial heap pages through the virtual-memory
  subsystem and obtains their physical frames from the PMM.
- A global kernel heap API for the rest of the kernel.
- Heap statistics and boot-time diagnostics.
- Unit tests for the pure allocator logic (host target, no QEMU).
- QEMU integration tests for heap initialization, allocation, deallocation,
  mapping, PMM accounting, and failure behavior.

## Heap Address Range

The Elvaris address-space design does not exist yet, so the heap range is a
**provisional** layout. It is chosen to be conflict-free with everything the
bootloader currently establishes, and it is expected to be revised when the
final address-space design is written.

Provisional layout (x86-64, 4-level page tables):

| Range | Size | Owner |
|---|---|---|
| `0x0000_0000_0000_0000` – `0x0000_7FFF_FFFF_FFFF` | 128 TiB | low half, dynamic bootloader mappings |
| `0xFFFF_8000_0000_0000` – `0xFFFF_8000_FFFF_FFFF` | 4 GiB | physical memory direct map (bootloader) |
| `0xFFFF_A000_0000_0000` – `0xFFFF_A003_FFFF_FFFF` | 64 MiB | **kernel heap (reserved)** |
| `0xFFFF_A004_0000_0000` – `0xFFFF_FFFF_FFFF_FFFF` | — | free for future kernel mappings |

Concretely:

```text
HEAP_VIRT_BASE   = 0xFFFF_A000_0000_0000
HEAP_RESERVED_LEN = 0x0400_0000            (64 MiB reserved range)
HEAP_MAPPED_PAGES = 256                    (1 MiB mapped during initialization)
HEAP_MAPPED_LEN   = 0x0010_0000            (1 MiB of the reserved range)
```

Why this range:

- It lies in the canonical high half and in a **dedicated PML4 entry (index
  320)**, so the heap never shares a top-level entry with the direct map
  (PML4 index 256) or with the bootloader's dynamic mappings (PML4 indices
  1–5: kernel image, framebuffer, kernel stack, boot information).
- The kernel image is a position-independent executable placed by the bootloader
  at a dynamically chosen address in the low half; the stack, framebuffer and
  boot information are placed by the bootloader in the same region. None of
  them can reach PML4 index 320.
- The bootloader never allocates dynamically above the first free PML4 entry, so
  index 320 is reserved in practice; the range is nevertheless validated at
  runtime (see *Safety Invariants*).
- The heap range is 4 KiB-aligned and 1 MiB in size, so the whole initially
  mapped part lives inside a single page table (PT index 0..255) and needs
  exactly three page-table frames.

The 64 MiB reserved range is larger than the initially mapped 1 MiB so that a
later milestone can grow the heap without relocating it.

## Virtual Memory Integration

The heap does not touch page tables. It asks the virtual-memory subsystem to
map pages, and it asks the PMM for the physical frames behind them.

```text
Heap allocator:
    chooses virtual addresses inside the heap range

Virtual-memory subsystem (vmm::mapper):
    walks the page-table hierarchy and maps virtual pages

PMM (pmm::phys_alloc):
    supplies physical frames

Bootloader:
    no longer manages ordinary heap allocations
```

Rules:

- The heap requests a *page* (4 KiB) at a time through the mapper. The mapper
  allocates the backing frame from the PMM and installs the page-table entry.
- The mapper allocates intermediate page-table frames from the PMM as well.
- The heap never writes to a page-table structure and never computes a physical
  address on its own.
- The mapper invalidates the TLB entry for every address it maps, so newly
  mapped pages are immediately usable.
- The mapper is the only component that knows about the physical-memory direct
  map; the heap works purely in virtual addresses.

Enabling the direct map: the kernel requests a physical-memory mapping from the
bootloader through its `.bootloader-config` section
(`mappings.physical_memory = Some(Mapping::FixedAddress(0xFFFF_8000_0000_0000))`).
Without it the kernel cannot dereference page tables at all, because the
bootloader's kernel page table contains no identity map. `BootInfo` reports the
resulting offset as `physical_memory_offset`, and the kernel refuses to
initialize the heap if it is absent.

Implementation notes:

- The physical frame address of a page-table entry is extracted with
  `FRAME_MASK` (bits 12..=51). Masking only the low twelve bits would leave
  the flag bits — notably the no-execute bit in bit 63 — inside the address
  and produce a non-canonical pointer.
- A new mapping invalidates the TLB entry of the affected address with
  `invlpg`. When an upper-level paging structure is modified (a new PDPT, PD
  or PT is installed), the whole TLB is flushed by reloading CR3, because a
  targeted invalidation does not necessarily reach the processor's
  paging-structure caches.
- The mapper never maps the frame at physical address zero. The bootloader
  reserves it, and the Rust core library assumes that address zero is not a
  valid pointer, so mapping it would turn a null-pointer bug into silent
  memory corruption. The first usable region reported by UEFI starts at
  physical address zero, so the PMM does hand out that frame; the mapper skips
  it and takes the next one. Because the PMM cannot reclaim frames, the
  skipped frame is wasted — 4 KiB in exchange for the guarantee that no
  mapping is ever backed by physical address zero. The boot diagnostics make
  the extra frame visible: the heap consumes one frame more than the number of
  heap pages plus the three page-table frames.

## Physical Memory Integration

- Every heap page is backed by a frame obtained from the PMM.
- The PMM is the only source of physical frames; the heap never fabricates a
  physical address.
- The PMM is a non-reclaiming bump allocator, so heap pages are never returned
  to the PMM. This is a documented limitation, not a hidden behaviour: the
  heap's page count only grows, and the reported statistics make that visible.
- The bootloader reports the frames it allocated itself (page tables, GDT, boot
  information, kernel stack) as `MemoryRegionKind::Bootloader`, and the PMM
  only allocates from `MemoryRegionKind::Usable` regions. Bootloader frames are
  therefore never handed to the heap.
- The kernel image and the ramdisk are likewise reported as bootloader-owned.

## Allocator Choice

**Chosen: a linked-list (free-list) allocator with a physically ordered,
doubly-linked list of blocks.**

Advantages:

- Supports deallocation, which the bump allocator does not.
- Reasonable amount of metadata handling for a small early kernel.
- Supports arbitrary power-of-two alignment, including page alignment.
- Coalescing of adjacent free blocks recovers most fragmentation.

Disadvantages that are accepted for this milestone:

- First-fit search is O(number of blocks) and fragmentation is possible.
- Metadata is stored inline in the heap, so the arena is not fully usable by
  the caller.
- The allocator is not SMP-safe; serialisation is the caller's responsibility
  (the kernel heap front-end serialises access with a minimal lock).

Rejected alternatives:

- **Bump allocator**: cannot reclaim memory, so it is not a complete kernel
  heap. It remains a valid *stage*, but it is not what milestone 5 delivers.
- **Slab allocator**: too specialised for a first general-purpose heap; it is
  a natural follow-up once kernel object types exist.

The allocator is split into two layers so that the allocation logic can be
tested on the host without any paging involved:

```text
heap::allocator::Arena   pure logic over a caller-supplied memory region
heap::KernelHeap         owns the arena, obtains pages from the VM subsystem
```

## Allocation and Deallocation Rules

- `Arena::alloc(size, align)` returns a pointer to `size` usable bytes aligned
  to `align`.
- `align` must be a power of two. Alignments of 16 bytes or less are satisfied
  naturally; larger alignments are satisfied by over-aligning inside the block.
- The allocator uses **first fit**: the first free block that can satisfy the
  request is used.
- A free block that is large enough is **split**; the remainder stays free. A
  remainder too small to hold a header plus a useful payload is absorbed into
  the allocated block.
- `Arena::dealloc(ptr, size)` returns the block to the free list and **coalesces**
  it with the physically adjacent free neighbours, in both directions.
- Freeing a pointer that was not produced by the allocator, freeing a pointer
  twice, or freeing a pointer that lies outside the arena is **rejected** with an
  error; it never corrupts the heap.
- Allocation sizes are byte counts; the allocator never rounds a request up to a
  page. Page-sized requests are ordinary allocations of `4096` bytes.
- The allocator is **not** required to be stable: a block's address does not
  change while it is allocated, but the heap base address is fixed for the
  lifetime of the heap.
- A **zero-sized allocation** succeeds. It returns a non-null, aligned pointer
  that owns no memory; it is placed at the end of the mapped region, so it is
  inside the reserved heap range but is not part of any block and cannot
  corrupt the allocator's metadata. Writing through it is a contract
  violation. Releasing it (`dealloc(ptr, 0)`) is a no-op.

## Alignment Requirements

| Request | Guarantee |
|---|---|
| byte (`u8`) | 1-byte aligned |
| `u16` | 2-byte aligned |
| `u32` | 4-byte aligned |
| `u64` | 8-byte aligned |
| pointer | 8-byte aligned |
| structure | the structure's own alignment (at least 16 for the general case) |
| page-aligned allocation | 4096-byte aligned |
| allocator metadata | 16-byte aligned, never inside a user allocation |

The block header is 32 bytes and 16-byte aligned, so every block payload
starts on a 16-byte boundary. For alignments greater than 16 bytes the
allocator over-aligns inside the block and stores a back-pointer immediately
before the returned address, so the block can always be recovered on free.

## Out-of-Memory Behavior

| Situation | Behavior |
|---|---|
| Heap region exhausted (no free block large enough) | `Err(HeapError::OutOfMemory)`, diagnostic printed, no pointer returned |
| PMM has no free frames | `MapError::OutOfMemory` from the mapper, propagated as `HeapError::OutOfMemory` |
| Virtual address space exhausted (page tables cannot be created) | `MapError::OutOfMemory`, propagated unchanged |
| A mapping fails midway through heap initialization | already-mapped pages are unmapped, the error is propagated, initialization fails |
| Invalid layout (alignment not a power of two, size overflow) | `Err(HeapError::InvalidAlignment)` / `Err(HeapError::SizeOverflow)` |
| Unexpected failure during required kernel initialization | controlled panic with a diagnostic, never a silent failure |

The allocator never returns an invalid or partially-initialised pointer. A
failed allocation returns an error; it does not return null and it does not
return a pointer outside the heap range.

## Safety Invariants

1. Every returned allocation lies within the heap range.
2. Every returned allocation satisfies its requested alignment.
3. Allocations do not overlap.
4. Deallocated blocks are not returned while still in use.
5. Heap metadata does not overlap user allocations.
6. Heap pages are mapped before use.
7. Physical frames are obtained only through the PMM.
8. Allocator failure never returns an invalid pointer.
9. The heap does not overlap existing kernel mappings.
10. Allocator behavior is defined for zero-sized allocations.

Supporting rules:

11. Freeing a pointer that the allocator did not produce, or freeing the same
    pointer twice, is detected and rejected.
12. A block that is in use is never merged into a free block.
13. The heap range is validated against the mappings that exist at
    initialization time; a conflict aborts initialization instead of silently
    overwriting a mapping.
14. The allocator's internal list is validated by a `validate()` routine that
    is used by the unit tests and by the boot-time self test.

## Testing Strategy

### Unit tests (host, pure allocator)

The allocator has its own unit test module, so the allocation logic is
exercised without any kernel environment. The suite covers:

- Basic allocation.
- Multiple allocations, and that they do not overlap.
- Alignment: 1, 2, 4, 8, 16, 32, 64, 128 and 4096.
- Deallocation returns memory to the allocator.
- Block reuse: a freed block is handed out again.
- Adjacent free blocks are coalesced.
- Fragmentation: many small allocations and frees do not corrupt the heap.
- Zero-sized allocation is defined and does not corrupt the heap.
- Exhaustion returns an error instead of a pointer.
- Invalid or oversized requests are rejected.
- Double free and foreign-pointer free are rejected.
- A size that does not match the block is rejected.
- The internal list stays consistent (`validate()`).

### Integration tests (QEMU)

`Kernel/tests/heap.rs` boots the kernel in QEMU and asserts:

- Heap initialization succeeds and reports the provisional range.
- The first allocation succeeds, the block is writable, and the contents are
  read back unchanged.
- A page-aligned allocation is really page aligned.
- Multiple allocations succeed; the heap is exhausted and reports the failure
  instead of returning an invalid pointer; the blocks are released again and
  the heap is empty and consistent afterwards.
- The heap pages are mapped: the mapper reads back the mapping of the heap
  base and the physical frame is a valid PMM frame.
- PMM statistics change by exactly the expected number of frames
  (heap pages + page-table frames).
- The QEMU boot still completes successfully, and the heap report appears
  after the boot information and the validated memory map.

`Kernel/tests/boot.rs` keeps the pre-existing boot test, which now shares the
QEMU harness in `Kernel/tests/common/mod.rs`. The harness captures the
complete serial output, so a missing marker can never be confused with a
marker that was cut off at the end of the pipe.

## Diagnostics

At boot the kernel prints:

- The physical-memory offset (direct map base) that makes page tables
  reachable.
- The heap virtual range, the reserved size and the number of mapped pages.
- The PMM frame counters before and after heap initialization, and the number
  of frames the heap consumed. The kernel checks that the number is at least
  the number of heap pages and at most that number plus the page-table frames
  the mapping needed plus the frame the mapper refuses to map; a mismatch is a
  controlled panic.
- The heap allocator statistics: total capacity, used bytes, free bytes,
  allocation and free counts, block count.
- The result of the boot-time heap self test (allocate, write, verify, free,
  exhaust, release, validate).

The exact serial markers the integration test relies on are:

```text
Elvaris kernel: heap: init ok
Elvaris kernel: heap: pmm_frames=260 ok
Elvaris kernel: heap: self_test: alloc ok
Elvaris kernel: heap: self_test: free ok
Elvaris kernel: heap: self_test: exhaustion ok
Elvaris kernel: heap: self_test ok
Elvaris kernel: heap: base=0xffffa00000000000 reserved_bytes=67108864 mapped_pages=256
Elvaris kernel: heap: frames=0x1000..0x595000
Elvaris kernel: heap: capacity=1048576 used=0 free=1048544 blocks=1
Elvaris kernel: --- end heap ---
```

Allocation failures print a diagnostic that names the failure (exhaustion,
invalid alignment, mapping failure) and the requested size/alignment.

The QEMU exit status is written to the `isa-debug-exit` device as a **single
byte**. The device terminates the emulator on the first write, so writing the
remaining three bytes of a 32-bit value raced with the shutdown and made the
reported host exit code nondeterministic. With one byte the code is
deterministic: 33 for a successful boot, 35 for a panic.

## Out of Scope

- User-space heaps
- Per-process allocators
- SMP synchronization (the heap front-end uses one minimal lock; the allocator
  internals are single-threaded)
- NUMA
- Swapping
- Demand paging
- Memory compression
- Garbage collection
- Slab caches
- Huge pages
- C library allocation
- Filesystem allocation APIs
- ARM64 heap integration
- Final virtual-address layout
- General-purpose memory reclamation in the PMM
- Heap growth (the heap is a fixed, initially mapped region in this milestone)

## Exit Criteria

- [x] Heap address range is documented
- [x] Heap does not overlap existing mappings
- [x] Heap pages are mapped through the VM subsystem
- [x] Physical frames come through the PMM
- [x] Allocator choice is documented
- [x] Alignment is correct
- [x] Allocations do not overlap
- [x] Deallocation behavior is defined
- [x] Out-of-memory behavior is defined
- [x] Unit tests pass
- [x] QEMU heap integration test passes
- [x] PMM statistics remain consistent
- [x] Existing boot and VM tests still pass
- [x] Limitations are documented

## Implementation Order

1. Define the heap address range
2. Define the allocator interface
3. Test allocator logic independently
4. Reserve and map the initial heap pages
5. Connect the allocator to virtual memory
6. Connect page allocation to the PMM
7. Test allocation and deallocation
8. Test exhaustion
9. Add QEMU diagnostics
10. Re-run all previous tests
11. Update the milestone checklist
12. Tag the completed milestone

**Central rule:** Do not allow the heap allocator to hide failures in the PMM or
virtual-memory subsystem. Every page allocation and mapping failure must remain
observable.
