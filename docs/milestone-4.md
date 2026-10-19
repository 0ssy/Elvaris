# Milestone 4: Virtual Memory

## Objective

Establish virtual memory management for the Elvaris kernel, building on the physical frame allocator. The goal is to safely inspect, extend, and validate x86-64 page tables to support kernel virtual address space mappings, without replacing the bootloader's address space prematurely.

## Current Bootloader Paging State

The bootloader (bootloader 0.11.x, UEFI) enables paging before transferring control to the kernel. The kernel must not assume physical and virtual addresses are identical. Key questions to answer during this milestone:
- Whether the kernel receives an active page table
- Whether the kernel is identity-mapped
- Whether the kernel is mapped at a higher virtual address
- Whether a physical-memory offset is available
- Whether the kernel stack is mapped
- Which mappings are safe to preserve
- Whether the kernel plans to replace or extend the bootloader’s page tables

For the first implementation, the kernel will preserve existing bootloader mappings and add only the mappings required.

## Scope

- x86-64 page-table inspection
- Page-table frame allocation
- Physical-to-virtual conversion
- Mapping one or more 4 KiB pages
- Reading and validating mappings
- Basic unmapping policy (if feasible)
- Page permissions
- Page-fault diagnostics
- Unit tests for address and flag calculations
- QEMU integration testing

## x86-64 Address-Space Model

The kernel operates in 64-bit long mode. Initial design accounts for the canonical address space and the standard four-level page table hierarchy. Virtual addresses are translated via page tables; the kernel must validate addresses and mappings.

## Page-Table Levels

Four-level hierarchy for x86-64 (4 KiB pages):
- PML4 (Page Map Level 4)
- PDPT (Page Directory Pointer Table)
- Page Directory (PD)
- Page Table (PT)
- 4 KiB page frame

Huge pages (2 MiB/1 GiB) are not in scope for this milestone.

## Mapping and Unmapping Rules

- Every mapping must have clear ownership.
- Page-table frames are allocated from the PMM.
- Mapped physical frames are owned by the entity that requested the mapping.
- Only authorized code may remove mappings.
- Unmapping returns frames to the PMM only if the mapping policy allows reclamation (noting current bump allocator limitation).
- If a mapping already exists, the operation must be rejected or handled according to policy.
- The bootloader’s existing mappings must be preserved unless explicitly replaced later.

## Physical-to-Virtual Address Translation

The kernel needs a reliable way to access page-table frames and physical memory. Translation decisions depend on the bootloader state:
- Use the bootloader-provided physical-memory offset if available and validated
- Fall back to identity mapping if applicable
- Use temporary mapping mechanisms if necessary

Translation invariant: A physical address may be converted to a virtual address only when the corresponding physical-memory mapping is known to exist.

## Kernel Memory Mapping

- Kernel code: readable and executable, not writable
- Kernel data: readable and writable, not executable
- User pages: inaccessible to user mode until user processes exist
- Kernel-only by default; user-accessible only when explicitly marked

## Page Permissions

Minimum permission set:
- Present/not-present
- Read/write
- User/supervisor
- Execute-disable (NX) behavior
- Page-size bits

Flags must be validated for each mapping.

## Frame Allocator Integration

- All page-table frames allocated via PMM
- Respect PMM ownership/lifetime rules
- Account for non-reclaiming bump allocator (no frame reclamation possible yet)
- Metadata placement documented if page tables are allocated

## Page-Fault Behavior

For Milestone 4, page faults will be handled diagnostically:
1. Print diagnostic information (faulting address, error code)
2. Identify fault cause (non-present, permission violation, invalid PTE, reserved bit, etc.)
3. Halt safely

Demand paging, COW, etc. are out of scope.

## Testing Strategy

- Unit tests for address calculation (canonical addresses, table indices)
- Unit tests for page-table entry flag construction/validation
- Unit tests for physical-to-virtual translation logic
- Integration tests via QEMU boot markers
- Verification that PMM and boot tests still pass

## Diagnostics

- Log current paging state on boot (if discoverable)
- Log page-table root location
- Log physical-memory offset if present
- Clear diagnostics on successful/failed mappings

## Out of Scope

- User processes and user address spaces
- Kernel heap allocation
- Demand paging, swapping, copy-on-write
- Huge pages (2 MiB/1 GiB)
- SMP, NUMA
- ARM64 page tables
- Memory compression
- Filesystem-backed mappings, memory-mapped files

## Exit Criteria

- [ ] Initial paging state is documented
- [ ] Physical-to-virtual translation is documented
- [ ] Page-table hierarchy is understood and documented
- [ ] Page-table frames come from the PMM
- [ ] 4 KiB mappings work
- [ ] Mapping flags are validated
- [ ] Kernel/user permissions are defined
- [ ] Invalid mappings are rejected
- [ ] Page-fault diagnostics work
- [ ] Unit tests cover address and flag logic
- [ ] QEMU boot test still passes
- [ ] PMM tests still pass
- [ ] Limitations are documented

## Implementation Order

1. Inspect the bootloader’s active page table
2. Document current mappings
3. Define page-table entry flags
4. Define physical-to-virtual translation
5. Connect page-table allocation to PMM
6. Map a controlled test page
7. Verify the mapping
8. Test invalid mappings
9. Add page-fault diagnostics
10. Add integration-test markers
11. Re-run all previous tests

**Central rule:** Do not replace the bootloader’s address space until you can inspect, extend, and validate page tables safely.
