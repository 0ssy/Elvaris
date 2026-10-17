# Elvaris Roadmap

## 1. Purpose

This roadmap describes the planned development order for Elvaris.

Elvaris is a long-term operating-system project targeting:

- x86-64
- ARM64
- Desktop systems
- Servers
- Embedded systems
- Native applications
- Future compatibility layers

The roadmap is organized into phases. Each phase should produce a testable result before the next phase becomes the primary focus.

The phases are not rigid deadlines. A phase is complete when its exit criteria are satisfied, not when a particular amount of time has passed.

## 2. Development Principles

Elvaris development should follow these principles:

1. Build the smallest useful system first.
2. Keep every milestone testable.
3. Prefer correctness before optimization.
4. Test in emulators before testing on physical hardware.
5. Keep x86-64 and ARM64 support visible from the beginning.
6. Avoid adding compatibility requirements too early.
7. Document major design decisions.
8. Use automated testing whenever possible.
9. Do not promise support for hardware or applications before testing it.
10. Keep unfinished features clearly marked as experimental.

## 3. Phase 0: Project Definition

### Objectives

Define the purpose, architecture, scope, and initial priorities of Elvaris before implementing kernel functionality.

### Documentation

Create and maintain:

- `README.md`
- `docs/vision.md`
- `docs/architecture.md`
- `docs/roadmap.md`
- `docs/security-model.md`
- `docs/compatibility.md`

### Decisions to Define

- Project name and identity
- Supported architectures
- Kernel architecture
- Initial boot strategy
- Initial target environments
- Development toolchain
- Testing strategy
- Initial system interface
- Security principles
- Compatibility priorities
- Contribution rules

### Exit Criteria

Phase 0 is complete when:

- The project vision is documented.
- The major architecture is documented.
- The initial scope is limited and realistic.
- The first kernel milestone is clearly defined.
- Security principles are documented.
- Compatibility expectations are documented.
- Development tools and test environments are selected.

## 4. Phase 1: Development Environment

### Objectives

Create a reproducible environment for building and testing Elvaris.

### Planned Work

- Select the host development operating systems.
- Select Rust and C toolchains.
- Select assemblers and linkers.
- Select x86-64 and ARM64 cross-compilation targets.
- Select QEMU machine configurations.
- Define build profiles.
- Define formatting and linting rules.
- Define debugging tools.
- Define logging conventions.
- Define test commands.
- Define artifact naming conventions.

### Initial Test Targets

The first targets should be virtual machines:

- x86-64 QEMU
- ARM64 QEMU

Physical hardware should not be required during the early phases.

### Exit Criteria

Phase 1 is complete when:

- The toolchain is documented.
- A developer can reproduce the required environment.
- Both architecture targets are defined.
- QEMU configurations are documented.
- Debugging and logging procedures are documented.
- The project has a clear process for reporting build failures.

## 5. Phase 2: Boot and Kernel Entry

### Objectives

Reach a Rust kernel entry point on both supported architectures.

### Planned Work

- Select or design the initial boot protocol.
- Define the boot information passed to the kernel.
- Define the kernel entry contract.
- Establish an initial stack.
- Establish basic CPU state.
- Initialize early console output.
- Add architecture identification.
- Add early panic handling.
- Add early serial logging.
- Record the memory map provided by the boot environment.

### Expected Result

The system should boot in an emulator and report:

- Kernel startup
- Target architecture
- Basic memory information
- Boot information status
- Kernel version or development identifier

### Exit Criteria

Phase 2 is complete when:

- x86-64 reaches the kernel entry point.
- ARM64 reaches the kernel entry point.
- Both targets produce diagnostic output.
- Kernel startup failures are observable.
- The boot interface is documented.
- The ker
