# Elvaris Vision

## 1. Purpose

Elvaris is a portable hybrid operating system designed for x86-64 and ARM64 systems.

Its long-term goal is to provide one coherent operating-system foundation that can serve three environments:

- Desktop computers
- Servers
- Embedded systems

Elvaris will begin as a command-line-first system. The initial focus will be on building a reliable kernel, clear system interfaces, hardware abstraction, process management, storage, and basic user-space services.

A graphical desktop, broad hardware support, and application compatibility will be developed later, after the core operating system is stable.

Elvaris will prioritize native applications first. Compatibility with POSIX, Linux, Windows, and other application ecosystems will be explored through compatibility layers, runtimes, translation interfaces, or virtualization where appropriate.

## 2. Main Goals

### 2.1 Security

Elvaris should provide strong security by default.

The system should aim to include:

- Process and address-space isolation
- Explicit permission boundaries
- Least-privilege services
- Secure application packaging
- Signed system updates
- Clear privilege escalation
- Protection against common memory-safety problems

Rust will be preferred for new system components where practical. Unsafe code, C code, and assembly will still be used where hardware or performance requirements justify them.

### 2.2 Reliability

A failure in one user-space service should not automatically crash the entire operating system.

Elvaris should support:

- Fault isolation
- Service restart and recovery
- Clear error reporting
- Defensive resource management
- Kernel panic diagnostics
- Reproducible bug reports
- Automated testing of critical components

### 2.3 Portability

Elvaris will support both x86-64 and ARM64.

Architecture-independent code should be kept separate from architecture-specific code. The design should make it possible to add other architectures in the future without rewriting the entire operating system.

The first supported environments will be virtual machines and carefully selected physical hardware.

### 2.4 Modularity

The operating system should be composed of replaceable components wherever practical.

Possible independently replaceable components include:

- Filesystem implementations
- Device drivers
- Networking services
- Storage services
- Audio services
- Graphics services
- Compatibility layers
- Package and update services

The kernel should contain only the functionality that requires kernel-level privileges, performance, or direct hardware control.

### 2.5 Performance

Elvaris should provide efficient use of system resources without sacrificing security or maintainability.

Performance goals include:

- Low unnecessary overhead
- Efficient process and thread scheduling
- Efficient system calls and inter-process communication
- Reasonable boot time
- Predictable resource usage
- Good performance on both powerful and constrained hardware

Performance decisions should be supported by measurements and benchmarks rather than assumptions.

### 2.6 Privacy

Elvaris should respect user privacy.

The system should not require unnecessary accounts, telemetry, or cloud services for basic operation.

Any network communication performed by the operating system should be documented and controllable by the user.

### 2.7 Developer Experience

Elvaris should be understandable and practical for developers.

The project should provide:

- Clear documentation
- Stable development tools
- Useful debugging output
- Reproducible builds
- Automated tests
- Consistent APIs
- Cross-compilation support
- Clear contribution guidelines

### 2.8 Long-Term Compatibility

Elvaris should eventually support more than only native Elvaris applications.

The compatibility strategy will be developed in stages:

1. Native Elvaris applications
2. C and Rust application support
3. POSIX-style applications
4. Selected Linux applications
5. Selected Windows applications
6. Additional runtimes where technically and legally feasible

Compatibility will not be treated as a promise until the required interfaces have been designed, implemented, tested, and maintained.

## 3. Non-Goals for the Initial Development Stage

These are not permanent limitations. They define what Elvaris will intentionally avoid attempting during its early development.

### 3.1 Supporting Every Device

The initial versions will not support every computer, GPU, wireless adapter, printer, storage controller, or peripheral.

Development will begin with:

- QEMU virtual machines
- A limited x86-64 target
- A limited ARM64 target
- Carefully selected physical hardware later

Hardware support will expand gradually.

### 3.2 Running Every Application

The initial versions will not run every Windows, Linux, macOS, Android, or other-platform application.

The first priority is to create a stable native application environment. Compatibility layers will be developed only after the underlying process, filesystem, networking, graphics, and system-call interfaces are mature enough to support them.

### 3.3 Building a Graphical Desktop Immediately

The initial system will be command-line-first.

A graphical desktop will not be the first milestone. It will be developed after the system can reliably:

- Boot
- Manage memory
- Run processes
- Handle input and output
- Provide storage
- Support basic user-space services

### 3.4 Supporting Every Filesystem

The early system will not attempt to support every existing filesystem.

It will begin with a simple, well-documented filesystem suitable for development and testing. Additional filesystems may be added after the storage interfaces are stable.

### 3.5 Supporting Every GPU

GPU support is a major engineering project.

Early graphics work, if introduced, will likely begin with:

- A framebuffer
- Software rendering
- A simple display abstraction
- QEMU-compatible virtual graphics hardware

Advanced GPU acceleration will be considered later.

### 3.6 Replacing Existing Operating Systems Immediately

Elvaris will not initially attempt to replace Linux, Windows, macOS, or other mature operating systems for general users.

Those systems have decades of development, extensive hardware support, large application ecosystems, and many specialized tools.

The initial purpose of Elvaris is to build a strong, understandable, and extensible foundation.

### 3.7 Supporting All Three Product Environments at Once

Elvaris is intended to support desktop, server, and embedded systems over time.

However, the project will not attempt to complete all three environments simultaneously during the earliest stages.

The shared kernel and core abstractions will be designed with all three in mind, while actual implementation will proceed incrementally.

## 4. Initial Target Users

The first users of Elvaris will most likely be:

- Operating-system developers
- Rust and C programmers
- Systems researchers
- Security researchers
- Embedded developers
- Server and infrastructure developers
- People interested in learning how operating systems work

General desktop users will become a target only after Elvaris has sufficient hardware support, application support, usability, and reliability.

## 5. Design Principles

Elvaris will follow these principles:

1. Keep kernel responsibilities clearly defined.
2. Prefer memory-safe Rust for new components where practical.
3. Keep unsafe code small, documented, and reviewed.
4. Separate architecture-independent and architecture-specific code.
5. Isolate failures whenever practical.
6. Prefer simple and testable designs over premature complexity.
7. Measure performance instead of guessing.
8. Document important technical decisions.
9. Maintain a clear boundary between the kernel and user space.
10. Avoid making compatibility promises before they can be tested.
11. Treat security as a fundamental design requirement, not a later feature.
12. Preserve the ability to replace major system components.
13. Keep development reproducible across supported architectures.
14. Build incrementally, with each milestone producing a testable result.

## 6. Initial Scope

The first usable development version of Elvaris should be able to:

- Boot on x86-64 in an emulator
- Boot on ARM64 in an emulator
- Identify the target architecture
- Initialize basic memory management
- Handle basic exceptions
- Provide kernel logging
- Start at least one user-space process
- Provide a command-line interface
- Read keyboard or console input
- Produce console output
- Provide a basic filesystem
- Load and execute a small native program
- Shut down or reboot cleanly

This initial version is not expected to be suitable for general-purpose daily use.

Its purpose is to prove that the core operating-system architecture works.

## 7. Long-Term Direction

After the initial command-line system is stable, Elvaris may expand to include:

- Networking
- Remote administration
- Server profiles
- Embedded system profiles
- Device management
- Multiple filesystems
- Package management
- Signed updates
- A graphical display system
- A window manager or desktop environment
- Audio support
- GPU acceleration
- POSIX compatibility
- Linux application support
- Windows application support
- Containers or lightweight virtualization
- Developer SDKs and application tooling

These features will be introduced only when the underlying architecture can support them safely and maintainably.

## 8. Definition of Success

Elvaris will not define success simply as having more features than existing operating systems.

The project will be successful if it creates an operating system that is:

- Understandable
- Secure by default
- Reliable under failure
- Portable across x86-64 and ARM64
- Usable in desktop, server, and embedded configurations
- Modular and maintainable
- Honest about its compatibility limits
- Welcoming to developers
- Capable of growing without losing architectural clarity

Elvaris aims to become better than existing systems in carefully chosen areas rather than attempting to be universally better at everything.
