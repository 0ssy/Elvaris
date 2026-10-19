# Milestone 4 Status

## Completed
- [x] vmm module skeleton (vmm/mod.rs)
- [x] Page table entry flags definitions (vmm/entry.rs) - Present, Writable, User, Accessed, Dirty, NX, etc.
- [x] Page table structures (vmm/page_table.rs) - constants for page size and entries per table
- [x] Address translation utilities (vmm/addr.rs) - phys_to_virt, virt_to_phys using physical_memory_offset
- [x] Fault diagnostics skeleton (vmm/fault.rs)
- [x] Unit test skeleton for vmm (vmm/tests.rs)
- [x] All existing tests still pass (13 PMM unit tests + 1 boot test + 1 vmm dummy test)

## In Progress / Not Yet Done
- [ ] Initial paging state is documented (bootloader state - CR3, offsets, identity mapping vs higher half)
- [ ] Physical-to-virtual translation is documented in detail
- [ ] Page-table hierarchy is understood and documented
- [ ] Page-table frames come from the PMM
- [ ] 4 KiB mappings work
- [ ] Mapping flags are validated
- [ ] Kernel/user permissions are defined
- [ ] Invalid mappings are rejected
- [ ] Page-fault diagnostics work (actual implementation beyond skeleton)
- [ ] Unit tests cover address and flag logic (more than dummy test)
- [ ] Limitations are documented

## Notes
- Following conservative strategy: preserve bootloader's existing mappings, extend only as needed
- Using bootloader_api::info::BootInfo fields (physical_memory_offset, recursive_index) as per bootloader 0.11.17
- Current PMM is non-reclaiming bump allocator - limitation already documented in PMM
