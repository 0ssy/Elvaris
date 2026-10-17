# Elvaris Compatibility Strategy

## 1. Purpose

This document defines how Elvaris will support applications, tools, interfaces, and hardware from other operating-system ecosystems.

Elvaris will not initially promise compatibility with every application. Compatibility will be developed in stages, beginning with a stable native platform.

The compatibility strategy must balance:

- Application support
- Security
- Performance
- Maintainability
- Legal requirements
- Development cost
- User expectations

## 2. Compatibility Principles

Elvaris will follow these principles:

1. Native applications come before compatibility layers.
2. Compatibility must not compromise kernel security.
3. Compatibility should be implemented in user space where practical.
4. Unsupported behavior must be documented.
5. Compatibility claims must be tested.
6. The kernel should not be designed around every external operating system.
7. Stable APIs are more valuable than short-term hacks.
8. Compatibility should be measured by real applications.
9. Translation and virtualization should be used where appropriate.
10. Legal and licensing requirements must be respected.

## 3. Compatibility Categories

Compatibility has several different meanings.

### 3.1 Source Compatibility

An application can be rebuilt from its source code for Elvaris with limited changes.

### 3.2 Binary Compatibility

An existing compiled application can run without being recompiled.

### 3.3 API Compatibility

Elvaris provides interfaces that behave similarly to another operating system.

### 3.4 ABI Compatibility

Elvaris provides compatible binary calling conventions, data layouts, executable formats, and system interfaces.

### 3.5 Hardware Compatibility

Elvaris supports hardware designed for another operating system.

### 3.6 Behavioral Compatibility

An application receives sufficiently similar behavior to operate correctly, even if the implementation is different.

These categories should not be confused. Supporting one does not automatically provide the others.

## 4. Compatibility Priorities

The planned compatibility order is:

1. Native Elvaris applications
2. C and Rust applications
3. POSIX-style applications
4. Selected Linux command-line applications
5. Selected Linux graphical applications
6. Selected Windows applications
7. Additional runtimes and platforms where practical

macOS, Android, and other ecosystems may be considered later, but they should not be part of the initial compatibility promise.

## 5. Native Elvaris Applications

Native applications are the first compatibility target because they define the platform Elvaris controls directly.

Native applications should receive:

- A documented executable format
- Process creation and termination
- Filesystem access
- Threading
- Inter-process communication
- Networking
- Time services
- Security permissions
- Package management
- Debugging support

Native applications should not depend on undocumented kernel behavior.

## 6. C Application Support

C support is important because a large amount of systems software is written in C.

Elvaris should eventually provide:

- A C ABI
- A C compiler toolchain
- Basic C runtime support
- Header files
- Standard data types
- Memory allocation
- File access
- Process interfaces
- Threading support
- Networking support
- Error handling

C support does not automatically mean that Linux or Windows binaries will run unchanged.

## 7. Rust Application Support

Rust is a primary development language for Elvaris.

Rust support should eventually include:

- Target definitions
- Standard-library support where practical
- Allocator integration
- Threading support
- File APIs
- Networking APIs
- Process APIs
- Debugging support
- Cross-compilation
- Package integration

The Elvaris Rust platform should clearly document which APIs are stable and which are experimental.

## 8. POSIX Compatibility

POSIX compatibility can make it easier to port command-line tools and server software.

A POSIX-style layer may provide:

- File descriptors
- Files and directories
- Pipes
- Processes
- Signals
- Threads
- Sockets
- Environment variables
- Standard input and output
- Common filesystem behavior
- C library interfaces

POSIX behavior may not map perfectly to Elvaris primitives.

The compatibility layer should document:

- Supported interfaces
- Unsupported interfaces
- Behavioral differences
- Threading differences
- Process-creation differences
- Signal behavior
- Filesystem differences
- Performance limitations

## 9. Linux Compatibility

Linux compatibility is a major project and should begin only after the native Elvaris platform is stable.

Potential approaches include:

- Source-level porting
- A Linux system-call translation layer
- A Linux user-space runtime
- Containers
- Lightweight virtual machines
- Full virtual machines
- Compatibility libraries

The appropriate method may differ between command-line and graphical applications.

### 9.1 Linux Executable Support

Linux binary compatibility may require:

- ELF executable loading
- Linux ABI handling
- Linux system-call behavior
- Linux signal behavior
- Linux thread behavior
- Linux filesystem expectations
- Dynamic-library loading
- Runtime-library compatibility

### 9.2 Linux Command-Line Applications

Command-line compatibility should be attempted before graphical compatibility.

Early targets may include:

- Shell utilities
- Text-processing tools
- Compilers
- Interpreters
- Network utilities
- Server programs

Each supported application should have compatibility tests.

### 9.3 Linux Graphical Applications

Graphical applications may require:

- Display protocols
- Window management
- Input systems
- Graphics libraries
- Font systems
- Audio systems
- Hardware acceleration
- Desktop integration

Graphical compatibility should not begin until Elvaris has a stable graphics architecture.

## 10. Windows Compatibility

Windows compatibility is a long-term project.

It may require support for:

- PE executable loading
- Win32 APIs
- Windows process behavior
- Windows thread behavior
- Windows synchronization primitives
- Windows filesystem behavior
- Registry behavior
- Dynamic-link libraries
- Graphics APIs
- Audio APIs
- Installer behavior

Windows compatibility should be implemented as a user-space compatibility environment wherever practical.

It should not require weakening Elvaris's kernel security model.

### 10.1 Windows Application Categories

Windows applications differ significantly.

Possible categories include:

- Console applications
- Traditional desktop applications
- Games
- Development tools
- System utilities
- Applications using specialized drivers
- Applications requiring proprietary services

Console applications are likely to be easier to support than applications requiring advanced graphics, anti-cheat systems, specialized drivers, or proprietary services.

### 10.2 Graphics Compatibility

Windows graphics compatibility may require translation between graphics APIs.

Potential challenges include:

- Different rendering models
- GPU driver requirements
- Shader translation
- Display synchronization
- Input behavior
- Window management
- Audio integration
- Performance expectations

Graphics compatibility should be developed independently from the basic kernel.

## 11. macOS Compatibility

macOS application compatibility should not be an initial goal.

Challenges may include:

- Proprietary APIs
- Application signing
- Framework behavior
- System services
- Graphics interfaces
- Hardware assumptions
- Licensing restrictions
- Platform-specific application bundles

Elvaris may later support open standards or portable applications that also run on macOS, but this is different from running arbitrary macOS binaries.

## 12. Android Compatibility

Android compatibility should not be an initial goal.

Android applications may depend on:

- Android Runtime
- Android framework APIs
- Binder-style IPC
- Mobile hardware interfaces
- Google or vendor services
- Touch and sensor systems
- Application sandbox rules

A future Android-oriented runtime could be considered separately from the core Elvaris desktop and server environments.

## 13. Executable Formats

Elvaris should begin with a clearly defined native executable format.

The format must define:

- File structure
- Entry point
- Sections
- Relocations
- Symbol information
- Dynamic libraries
- Architecture identification
- Memory permissions
- Debugging information
- Versioning

Elvaris may use existing formats where practical, but it should not inherit incompatible behavior without documenting it.

Possible formats may include:

- ELF for initial systems compatibility
- A native Elvaris format later
- PE loading for Windows compatibility
- Other formats through user-space loaders

## 14. Application Binary Interface

The Elvaris ABI should define:

- Calling conventions
- Data type sizes
- Alignment
- Register usage
- System-call conventions
- Error values
- Thread-local storage
- Stack behavior
- Process startup data
- Dynamic linking
- Symbol naming
- Versioning

The ABI should be architecture-specific where necessary but expose a consistent application model on x86-64 and ARM64.

## 15. System-Call Compatibility

Elvaris should not copy another operating system's system calls without understanding their semantics.

The native system-call interface should be designed for:

- Security
- Clear ownership
- Explicit permissions
- Stable behavior
- Portability
- Testability

Compatibility layers may translate external system calls into native Elvaris operations.

A compatibility layer should not expose undocumented kernel internals.

## 16. Filesystem Compatibility

Applications may expect different filesystem behavior.

Compatibility concerns include:

- Path separators
- Case sensitivity
- File permissions
- Symbolic links
- File locking
- Timestamps
- File attributes
- Executable permissions
- Device files
- Special files
- Maximum path lengths
- Unicode behavior

Elvaris should define native filesystem behavior clearly and provide compatibility behavior only where it can be maintained safely.

## 17. Networking Compatibility

Applications may expect different networking behavior.

Compatibility may involve:

- Socket APIs
- Address structures
- Name resolution
- Connection behavior
- Timeouts
- Firewall behavior
- Interface naming
- Routing
- Network permissions

The native networking interface should remain independent from any single external operating system.

## 18. Device and Hardware Compatibility

Hardware compatibility requires more than recognizing a device.

It may require:

- Device discovery
- Firmware
- Drivers
- Power management
- Interrupt handling
- DMA control
- Configuration tools
- User-space interfaces
- Security permissions

Initial hardware support should focus on:

- QEMU virtual devices
- Serial devices
- Virtual storage
- Virtual networking
- Basic input
- Simple display output

Physical hardware support should be added according to documented priorities.

## 19. Compatibility and Security

Compatibility layers must follow Elvaris security rules.

They should:

- Run with restricted permissions
- Request access explicitly
- Avoid unrestricted device access
- Translate external privileges carefully
- Protect native processes
- Isolate compatibility environments
- Prevent unsupported calls from bypassing security
- Log relevant security failures

An application should not gain administrator-level access merely because its original operating system expected broad authority.

## 20. Compatibility Testing

Compatibility must be tested using real applications and controlled test suites.

Tests should measure:

- Whether the application starts
- Whether core features work
- Whether files are handled correctly
- Whether networking works
- Whether graphics work
- Whether performance is acceptable
- Whether permissions are respected
- Whether failures are recoverable
- Whether behavior is consistent across x86-64 and ARM64

Each compatibility target should have a support level.

## 21. Support Levels

Elvaris may classify applications using the following levels.

### Level 0: Unsupported

The application is known not to work or has not been tested.

### Level 1: Starts

The application launches but may have major limitations.

### Level 2: Basic Functionality

Core functions work, but some features are unavailable or unreliable.

### Level 3: Usable

The application is useful for ordinary supported workflows.

### Level 4: Well Supported

The application has broad test coverage and reliable behavior.

### Level 5: Native Integration

The application is integrated with Elvaris features such as permissions, packaging, updates, notifications, and system settings.

Support levels must be based on documented tests rather than informal reports alone.

## 22. Compatibility Performance

Compatibility may introduce overhead.

Performance evaluation should compare:

- Native execution
- Translated execution
- Virtualized execution
- Application startup time
- CPU usage
- Memory usage
- Storage performance
- Graphics performance
- Network performance
- Battery or power usage

Optimization should follow profiling rather than assumptions.

## 23. Licensing and Legal Requirements

Compatibility work must respect:

- Software licenses
- Copyright
- Trademark restrictions
- Patent concerns
- Redistribution requirements
- Reverse-engineering laws
- Proprietary interface restrictions
- Third-party runtime licenses

The project should maintain a clear record of external code, specifications, and licenses.

Legal uncertainty should be reviewed before distributing compatibility components.

## 24. Compatibility Roadmap

### Stage 1: Native Platform

Support native Elvaris applications written in Rust and C.

### Stage 2: Portable Source Applications

Port selected open-source applications to Elvaris.

### Stage 3: POSIX-Style Environment

Support common command-line tools and server applications through POSIX-style interfaces.

### Stage 4: Linux Command-Line Compatibility

Test selected Linux applications through translation, porting, containers, or virtualization.

### Stage 5: Linux Graphical Compatibility

Support selected graphical applications after the display and graphics systems are stable.

### Stage 6: Windows Compatibility Research

Study executable loading, API translation, graphics translation, and application behavior.

### Stage 7: Selected Windows Applications

Support carefully chosen Windows applications with documented limitations.

### Stage 8: Additional Runtimes

Evaluate other application ecosystems only after the core compatibility systems are maintainable.

## 25. Compatibility Success Criteria

The compatibility strategy will be considered successful when:

- Native applications have stable APIs.
- C and Rust developers can build applications.
- Selected POSIX applications can be ported.
- Compatibility layers do not weaken system security.
- Supported applications have documented support levels.
- Compatibility behavior is tested on x86-64 and ARM64 where possible.
- Unsupported behavior is clearly communicated.
- Compatibility components can be updated independently.
- The native operating-system architecture remains understandable.
- Compatibility work does not prevent kernel development from progressing.
