# Device-Tree

## Objective

Provide a controlled hardware-description interface for embedded platforms.

## Device-tree support

If the selected target uses a device tree, support:

- Node traversal
- Compatible-string matching
- Register ranges
- Interrupt descriptions
- Clock information
- Memory regions
- GPIO information where required
- Status fields
- Child-device relationships
- Chosen-node data
- Bootargs

Raw, unchecked device-tree values are not exposed directly to drivers.

## Validation

Validate:

- Node offsets
- Property lengths
- String termination
- Address-cell sizes
- Size-cell sizes
- Interrupt-cell sizes
- Register-range overflow
- Invalid references
- Duplicate or conflicting resources
- Unsupported compatible strings

Malformed hardware descriptions must not crash the kernel.

## Hardware binding

Drivers match hardware using documented identifiers:

- compatible
- vendor ID
- product ID
- bus type

Avoid matching only on physical addresses.

## Exit criteria

- [ ] Device-tree format documented
- [ ] Device-tree parser works
- [ ] Required nodes discovered
- [ ] Invalid properties rejected
- [ ] Resource ranges validated
- [ ] Driver matching works
- [ ] Unsupported devices reported safely
- [ ] Device-tree tests pass
