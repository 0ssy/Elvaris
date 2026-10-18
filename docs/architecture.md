
# Elvaris Architecture

## 1. Overview

Elvaris will use a hybrid-kernel architecture.

The kernel will provide the core mechanisms that require privileged access, strong performance, or direct hardware control. Other components will run in user space whenever practical so they can be isolated, restarted, replaced, and developed independently.

Elvaris will support:

- x86-64
- ARM64
- Desktop systems
- Servers
- Embedded systems
- Native Elvaris applications
- Future compatibility layers

The initial user interface will be command-line based.

## 4. Provisional Decisions (for review)

The following remain provisional rather than final:

- Exact bootloader choice (existing bootloader adopted; specific product TBD).
- Exact Rust target triples (provisional: `x86_64-unknown-none`, `aarch64-unknown-none`).
- Build host: WSL2 vs native Windows (provisional: WSL2).
- Hybrid-kernel split point between kernel and user space (will be refined as services are designed).
- Whether ARM64 is built in parallel later or sequentially (provisional: sequentially, after x86-64 Milestone 1).
- Embedded/server/desktop profile differences (deferred; shared core first).

Uncertain decisions are marked provisional and will be revisited at each milestone exit review.


The architecture should provide:

- Clear separation between kernel space and user space
- Process and address-space isolation
- Hardware portability
- Fault isolation
- Efficient communication between system components
- Support for desktop and constrained systems
- Stable interfaces between major subsystems
- A path toward application compatibility
- Debuggability during early development
- Replaceable system components

The architecture is intended to evolve. Early implementations may use simpler designs, but they should avoid preventing future modularity.

## 3. System Layers

Elvaris will be divided into several logical layers:

```text
Applications
    │
Application libraries and compatibility APIs
    │
User-space system services
    │
System-call and IPC interfaces
    │
Hybrid kernel
    │
Architecture-specific hardware support
    │
Physical hardware
```

## 4. Applications

Applications are programs that run outside the kernel.

They must not access hardware or kernel memory directly. Applications communicate with the operating system through system calls, libraries, and user-space services.

Examples include:

- Shells
- Text editors
- System utilities
- Network programs
- Server applications
- Embedded applications
- Future graphical applications

Applications should run with the minimum permissions required for their tasks.

## 5. Application Libraries

Libraries provide reusable functionality to applications.

Possible libraries include:

- Elvaris standard library
- C library compatibility layer
- Rust support
- Filesystem APIs
- Networking APIs
- Threading APIs
- Graphics APIs
- Security APIs
- Permissions APIs

Libraries should not be confused with kernel functionality. A library may provide a convenient interface while internally using system calls.

## 6. User-Space Services

User-space services provide operating-system functionality outside the kernel.

Possible services include:

- Initial process manager
- Console service
- Filesystem service
- Device manager
- Network service
- Logging service
- Time service
- Package service
- Audio service
- Graphics service
- Compatibility services

A service should run with only the permissions it needs.

Where practical, the system should be able to restart a failed service without restarting the entire operating system.

## 7. Hybrid Kernel

The kernel will contain core mechanisms that require privileged access or efficient execution.

Potential kernel responsibilities include:

- CPU initialization
- Memory protection
- Virtual memory
- Physical memory management
- Scheduling
- Threads and processes
- Inter-process communication primitives
- System-call entry and validation
- Interrupt handling
- Exception handling
- Basic security enforcement
- Low-level device access
- Hardware abstraction
- Timekeeping

The exact division between kernel and user space may change as the project develops.

The kernel should remain as small and understandable as possible while still meeting performance and hardware requirements.

## 8. Hardware Abstraction

Hardware-specific code will be isolated behind common interfaces.

The rest of the operating system should not need to know whether it is running on x86-64 or ARM64.

Examples of hardware abstractions include:

- CPU initialization
- Page-table management
- Interrupt handling
- Timer access
- Context switching
- Atomic operations
- Console output
- Device discovery
- Power management

Architecture-independent code should use these abstractions instead of directly accessing architecture-specific features.

## 9. Kernel Subsystems

The kernel will be divided into major subsystems.

### 9.1 Architecture Layer

The architecture layer contains code specific to a CPU architecture.

For x86-64, this may include:

- Boot entry
- CPU mode setup
- Page tables
- Interrupt controllers
- CPU exceptions
- Context switching
- Architecture-specific registers
- Hardware timers
- CPU feature detection

For ARM64, this may include:

- Exception levels
- Exception vectors
- Translation tables
- Interrupt controllers
- Generic timers
- Context switching
- Device-tree interaction
- CPU feature detection

Architecture-specific code should not spread throughout the rest of the kernel.

### 9.2 Memory Management

Memory management controls how physical and virtual memory are used.

It will eventually include:

- Physical-frame allocation
- Virtual-address management
- Page-table management
- Kernel heap allocation
- User-space memory mappings
- Shared memory
- Memory protection
- Memory-mapped files
- Optional memory reclamation
- Memory statistics

The memory manager must preserve isolation between processes and prevent user programs from modifying kernel memory.

### 9.3 Processes and Threads

A process represents an isolated program environment.

A thread represents a sequence of execution within a process.

The process subsystem will eventually manage:

- Process creation
- Process termination
- Threads
- Address spaces
- Handles and resources
- Parent-child relationships
- Signals or event notifications
- Environment data
- Program loading
- Process permissions

The initial implementation may support only a small subset of these features.

### 9.4 Scheduler

The scheduler decides which runnable thread receives processor time.

Initial scheduling should prioritize:

- Correctness
- Isolation
- Predictable behavior
- Easy debugging
- Fair access to the processor

Later scheduling improvements may include:

- Priorities
- CPU affinity
- Power-aware scheduling
- Real-time scheduling
- Different policies for desktop, server, and embedded profiles

Scheduling decisions should be measured and tested rather than based only on assumptions.

### 9.5 Inter-Process Communication

Inter-process communication, or IPC, allows isolated processes and services to communicate.

Possible IPC mechanisms include:

- Message passing
- Shared memory
- Channels
- Events
- Signals
- Sockets
- Resource handles

IPC should include security checks so a process cannot communicate with resources it does not have permission to access.

The initial IPC design should prioritize correctness, clear ownership, and easy debugging.

### 9.6 System Calls

System calls provide the controlled entry point from user space into the kernel.

System calls should:

- Validate all user-provided data
- Check permissions
- Avoid exposing kernel memory
- Return well-defined errors
- Be documented
- Be versioned carefully
- Avoid unnecessary architecture-specific behavior

The first system calls will likely be related to:

- Process control
- Memory mapping
- Console input and output
- File access
- IPC
- Time
- Thread management

### 9.7 Interrupts and Exceptions

Interrupts notify the kernel about hardware events.

Exceptions notify the kernel about processor-detected conditions.

The system must distinguish between:

- Recoverable application errors
- Invalid memory access
- System calls
- Timer events
- Device interrupts
- Fatal kernel errors

Unexpected kernel faults should produce useful diagnostic information, including:

- Processor state
- Fault address
- Current process
- Current thread
- Call stack where possible
- Relevant kernel logs

### 9.8 Drivers

Drivers provide access to hardware devices.

The driver design should support:

- Device discovery
- Device initialization
- Device shutdown
- Interrupt handling
- Data transfer
- Error reporting
- Resource ownership
- Permission enforcement
- Device removal where supported

Drivers may run in the kernel or user space depending on performance, hardware requirements, and safety considerations.

Early development should focus on a small number of virtual devices and simple physical hardware.

## 10. Boot Architecture

Elvaris will initially target virtual machines before broad physical-hardware support.

The boot process should provide the kernel with consistent information about:

- Available memory
- CPU details
- Hardware devices
- Boot arguments
- Display or console output
- Initial storage
- Firmware information

The kernel should not depend unnecessarily on one specific bootloader.

### 10.1 x86-64 Boot

The x86-64 boot process will eventually need to handle:

- Firmware handoff
- Bootloader interaction
- CPU mode preparation
- Memory-map discovery
- Framebuffer or console discovery
- Kernel loading
- Initial page tables
- Interrupt-controller setup
- Transfer to the Rust kernel entry point

### 10.2 ARM64 Boot

The ARM64 boot process will eventually need to handle:

- Firmware or bootloader handoff
- Exception-level setup
- Memory discovery
- Device-tree or equivalent hardware description
- Timer setup
- Interrupt-controller discovery
- Kernel loading
- Initial translation tables
- Transfer to the kernel entry point

The boot interface should provide the kernel with a consistent boot-information structure regardless of architecture.

## 11. Kernel and User-Space Boundary

The boundary between kernel space and user space is one of the most important parts of Elvaris.

Kernel space will have access to:

- Processor control
- Kernel memory
- Page tables
- Hardware resources
- Interrupts
- Security enforcement

User space will contain:

- Applications
- Shells
- Most system services
- Filesystem services where practical
- Networking services where practical
- Compatibility layers
- Package management
- Graphical services

The boundary should be crossed only through controlled mechanisms such as:

- System calls
- IPC
- Shared-memory regions
- Device interfaces
- Controlled handles

## 12. User-Space Service Arrangement

The first user-space services may be arranged as follows:

```text
init
├── console service
├── process service
├── filesystem service
├── device manager
└── logging service
```

The exact arrangement may change as implementation experience is gained.

The initial system may temporarily keep some functions in the kernel for simplicity. Moving functionality into user space should happen when the required interfaces are stable enough.

## 13. Storage and Filesystems

Elvaris will separate storage devices from filesystem implementations.

The storage layer will eventually provide access to:

- Virtual disks
- Physical disks
- Flash storage
- Memory-backed storage
- Embedded storage devices

The filesystem layer will provide:

- Files
- Directories
- Metadata
- Permissions
- File handles
- Mounting
- Caching
- Persistent storage

Early development should use an in-memory filesystem or a deliberately simple filesystem before supporting complex production filesystems.

The storage design should avoid making applications dependent on a particular physical disk or filesystem implementation.

## 14. Networking

Networking will be developed after the process, memory, and basic device interfaces are stable.

The networking architecture may include:

- Network-device drivers
- Packet buffers
- Link-layer support
- IPv4
- IPv6
- Transport protocols
- Sockets
- Routing
- Firewalling
- User-space networking services

Networking should support different profiles for desktop, server, and embedded environments.

The design should make it possible to use networking locally, remotely, or not at all depending on the system profile.

## 15. Security Boundaries

Important security boundaries will include:

- Kernel and user space
- One process and another process
- One service and another service
- Applications and devices
- Applications and files
- Privileged and unprivileged users
- Native applications and compatibility layers

Every boundary should define:

- What can cross it
- How data is validated
- Which permissions are required
- How failures are handled
- How the boundary is tested
- How access is logged where appropriate

The detailed security model will be documented separately in `docs/security-model.md`.

## 16. Architecture Profiles

Elvaris will share a common core while supporting different system profiles.

### 16.1 Desktop Profile

The desktop profile may include:

- Interactive shell
- Graphics
- Audio
- Networking
- Wi-Fi
- User sessions
- Graphical applications
- Automatic updates
- Hardware power management

### 16.2 Server Profile

The server profile may include:

- Remote administration
- Networking
- Storage management
- Service supervision
- Logging
- Resource isolation
- Headless operation
- Monitoring
- Backup services
- High-availability features

### 16.3 Embedded Profile

The embedded profile may include:

- Small system images
- Read-only storage
- Device-tree support
- Limited services
- Watchdog support
- Deterministic scheduling
- Low memory usage
- Hardware-specific configuration
- Fast startup
- Field-update support

These profiles should share the same fundamental kernel interfaces where practical.

## 17. Portability Rules

To preserve portability:

1. Architecture-specific code must remain isolated.
2. Generic kernel code must use architecture-independent interfaces.
3. Hardware assumptions must be documented.
4. Both x86-64 and ARM64 should remain supported targets.
5. Tests should identify architecture-specific behavior.
6. Endianness assumptions should be explicit.
7. Alignment requirements should be documented.
8. Atomic and memory-ordering behavior must be understood.
9. Architecture-specific optimizations should have generic fallbacks where practical.
10. New subsystems should be evaluated on both target architectures.
11. Hardware-specific behavior should not leak into application APIs.
12. Build configurations should identify unsupported features clearly.

## 18. Application Compatibility Architecture

Elvaris will develop compatibility in stages.

### 18.1 Native Applications

Native applications will use Elvaris system interfaces and application libraries.

They should receive:

- Stable executable loading
- Process management
- Filesystem access
- Networking
- Threading
- Security permissions
- Package management

### 18.2 POSIX Compatibility

A POSIX-style compatibility layer may provide support for:

- File descriptors
- Processes
- Pipes
- Signals
- Sockets
- Threads
- Standard filesystem behavior
- C library interfaces

### 18.3 Linux Compatibility

Linux compatibility may require:

- Linux system-call translation
- ELF loading
- Linux filesystem behavior
- Common library support
- Linux device behavior
- Containers or virtual machines for difficult applications

### 18.4 Windows Compatibility

Windows compatibility is a separate, long-term project that may require:

- PE executable loading
- Win32 API support
- Windows filesystem behavior
- Windows process and thread semantics
- Registry behavior
- Graphics API translation
- Installer support

Compatibility layers should not be included in the initial kernel design unless they affect fundamental interfaces.

## 19. Provisional Decisions

The following decisions are provisional and may change:

- The exact kernel-to-user-space boundary
- The initial boot protocol
- The filesystem design
- The system-call ABI
- The IPC mechanism
- The driver placement model
- The executable format
- The scheduler design
- The package format
- The graphics architecture
- The networking architecture

Any major change should be recorded in project documentation so the reasoning remains visible.

## 20. Architectural Success Criteria

The architecture will be considered successful when it can:

- Support x86-64 and ARM64 without duplicating the entire kernel
- Isolate user applications from the kernel
- Start and manage user-space processes
- Provide controlled IPC
- Recover from selected user-space service failures
- Support command-line applications
- Add desktop, server, and embedded profiles without creating separate operating systems
- Provide stable foundations for future compatibility layers
- Keep architecture-specific code isolated
- Remain understandable and maintainable as the project grows

Elvaris will initially use the Rust bootloader crate to load an x86-64 kernel in QEMU through UEFI.

The bootloader is an initial development dependency, not a permanent architectural commitment.

The kernel will define an architecture-independent boot-information interface so that an ARM64 boot path can be added later.

A custom Elvaris bootloader may be developed after the kernel has reached a stable command-line milestone.
