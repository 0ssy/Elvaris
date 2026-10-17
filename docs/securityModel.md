# Elvaris Security Model

## 1. Purpose

This document defines the security goals, boundaries, assumptions, and planned protections for Elvaris.

Security is a core architectural requirement. It should not be added only after the operating system has been implemented.

Elvaris will target:

- x86-64
- ARM64
- Desktop systems
- Servers
- Embedded systems
- Native applications
- Future compatibility layers

The security model will evolve as the operating system gains new features, but fundamental principles should remain stable.

## 2. Security Goals

Elvaris should aim to:

- Protect kernel memory from user programs.
- Isolate processes from one another.
- Restrict services to the resources they need.
- Prevent unauthorized access to files and devices.
- Make privilege boundaries explicit.
- Reduce the impact of software failures.
- Protect system updates.
- Provide useful security diagnostics.
- Avoid unnecessary data collection.
- Make security behavior understandable to users and developers.

Security should be evaluated through design review, testing, measurement, and documented assumptions.

## 3. Security Principles

Elvaris will follow these principles:

1. Least privilege
2. Secure defaults
3. Defense in depth
4. Explicit authorization
5. Strong isolation
6. Minimized privileged code
7. Fail safely
8. Validate untrusted input
9. Keep security decisions visible
10. Reduce attack surface
11. Prefer memory-safe implementation where practical
12. Make security failures observable
13. Do not trust compatibility layers by default
14. Treat updates as security-sensitive operations

## 4. Threat Model

The threat model describes the types of threats Elvaris should consider.

### 4.1 Untrusted Applications

Applications may be buggy, compromised, or intentionally malicious.

The operating system must assume that an application may attempt to:

- Read another process's memory
- Modify kernel memory
- Access unauthorized files
- Access devices directly
- Exhaust system resources
- Impersonate another process
- Abuse system calls
- Escape its permissions
- Interfere with other applications

### 4.2 Untrusted Input

Elvaris may receive input from:

- Applications
- Users
- Filesystems
- Network packets
- Devices
- Firmware
- Bootloaders
- External storage
- Compatibility layers

All such input should be treated as potentially malformed or malicious.

### 4.3 Compromised Services

A user-space service may be compromised without the entire kernel being compromised.

The architecture should reduce the damage caused by a compromised service by limiting:

- Memory access
- Device access
- File access
- Network access
- IPC access
- Privilege level

### 4.4 Malicious or Faulty Devices

Hardware and device firmware may behave incorrectly or maliciously.

Drivers should not automatically receive unrestricted access to every resource unless that access is required.

### 4.5 Physical Attackers

Physical access may allow an attacker to:

- Remove storage
- Modify boot media
- Inspect memory
- Access debugging interfaces
- Replace hardware
- Interrupt startup

Physical security is outside the complete control of the operating system, but Elvaris may later support secure boot, disk encryption, and hardware-backed security features.

## 5. Trust Boundaries

The most important security boundaries are:

- Firmware and bootloader
- Bootloader and kernel
- Kernel and user space
- One process and another process
- One service and another service
- Applications and devices
- Applications and filesystems
- Compatibility layers and native services
- Users and privileged services
- Local users and remote clients

Every boundary should define:

- The data crossing the boundary
- The authority crossing the boundary
- The validation required
- The failure behavior
- The logging behavior
- The tests required

## 6. Kernel and User-Space Isolation

The kernel is the most privileged software in Elvaris.

User programs must not be able to:

- Read kernel memory
- Write kernel memory
- Modify page tables without authorization
- Disable security controls
- Access hardware directly without permission
- Execute privileged instructions
- Change another process's address space
- Bypass system-call validation

The kernel must validate all user-provided addresses, sizes, handles, identifiers, and structures.

## 7. Process Isolation

Each process should have an isolated address space.

A process should not be able to access another process's memory unless an explicit mechanism allows it.

Possible controlled sharing mechanisms include:

- Shared-memory regions
- IPC buffers
- Message channels
- File-backed mappings
- Device mappings
- Explicit resource handles

Shared resources must define:

- Ownership
- Lifetime
- Access permissions
- Synchronization requirements
- Revocation behavior
- Failure behavior

## 8. Threads and Resources

Threads belong to processes and should inherit only explicitly defined permissions.

The operating system must define how permissions behave when:

- A thread is created
- A process forks or clones
- A process executes a new program
- A process creates a child
- A service restarts
- A handle is transferred
- A process terminates

Resource lifetime must be tracked so that terminated processes do not leave unauthorized access behind.

## 9. Capabilities and Handles

Elvaris may use capability-based handles for access to operating-system resources.

A capability represents authority to perform specific operations on a resource.

Possible capability resources include:

- Files
- Directories
- Processes
- Threads
- Memory regions
- Devices
- IPC channels
- Network sockets
- Timers
- Shared-memory objects

A capability should define:

- The resource
- The allowed operations
- The owning process
- The lifetime
- Whether it can be transferred
- Whether it can be revoked

The design should avoid giving applications broad ambient authority when a narrower capability is sufficient.

## 10. Privileges

Privileged operations should require explicit authorization.

Possible privileged operations include:

- Loading kernel components
- Accessing protected devices
- Changing system configuration
- Managing other processes
- Mounting filesystems
- Creating users
- Changing security policy
- Configuring networking
- Installing system updates
- Managing power

Privilege should be divided where practical instead of assigning one service unlimited authority.

## 11. System-Call Security

System calls are a major attack surface.

Every system call should:

- Validate all arguments.
- Validate pointer ranges.
- Validate buffer sizes.
- Validate object types.
- Check permissions.
- Handle integer overflow.
- Handle invalid handles.
- Avoid exposing uninitialized data.
- Return defined error values.
- Avoid holding locks unnecessarily.
- Be safe under concurrent use.

System calls should be documented before they are considered stable.

## 12. Memory Safety

Rust should be preferred for new components where practical.

Unsafe Rust, C, and assembly may still be required for:

- CPU startup
- Context switching
- Interrupt entry
- Device access
- Foreign-function interfaces
- Performance-critical code
- Existing ecosystem support

Unsafe code should be:

- Small
- Isolated
- Documented
- Reviewed
- Tested
- Clear about its invariants

Memory-safety problems should be treated as security issues when they can cross a trust boundary.

## 13. Kernel Attack Surface

The kernel attack surface should be minimized.

This includes:

- System calls
- Interrupt handlers
- Device drivers
- Filesystem code
- Network code
- Memory-management code
- Boot parsing
- Debugging interfaces
- Module loading
- Compatibility interfaces

Features that do not require kernel privilege should be considered for user-space implementation.

## 14. Driver Security

Drivers can access hardware and may require high privilege.

The driver architecture should consider:

- Running drivers in user space where practical
- Restricting DMA access
- Validating device-provided data
- Limiting device mappings
- Handling device removal
- Isolating faulty devices
- Restricting driver permissions
- Auditing driver interfaces
- Disabling unused drivers

Drivers should not receive permissions unrelated to their device function.

## 15. Filesystem and Storage Security

The storage system should protect:

- File contents
- Directory structure
- Ownership
- Permissions
- Metadata
- Mount configuration
- Device access
- Temporary files
- System files

The filesystem design should consider:

- Path traversal
- Symbolic-link behavior
- Race conditions
- Unauthorized mounts
- Device files
- Set-user-ID behavior
- File replacement
- Metadata corruption
- Incomplete writes
- Recovery after power loss

Storage encryption may be added later, but the security model should leave room for it.

## 16. Networking Security

Networked systems should assume that network input is hostile.

Networking components should consider:

- Malformed packets
- Resource exhaustion
- Unauthorized connections
- Address spoofing
- Replay attempts
- Denial-of-service attacks
- Insecure services
- Weak authentication
- Information leakage

The system should provide:

- Clear network permissions
- Service binding controls
- Firewall support
- Secure remote administration
- Protected credentials
- Useful network logs

## 17. Application Sandboxing

Applications should run with limited access by default.

A sandbox may restrict:

- Filesystem paths
- Network access
- Device access
- Process inspection
- Inter-process communication
- Microphone and camera access
- Location information
- Sensitive system information

Sandbox policies should be understandable to developers and users.

Compatibility applications should receive no more authority than required for their supported behavior.

## 18. Service Isolation

User-space services should have separate identities and permissions where practical.

A service should not automatically receive:

- Full filesystem access
- Access to all devices
- Access to all processes
- Unlimited network permissions
- System-administration authority

Services should have:

- Explicit startup requirements
- Explicit dependencies
- Restricted capabilities
- Resource limits
- Failure handling
- Logging
- Update procedures

## 19. Resource Exhaustion

Applications and services may exhaust:

- Memory
- CPU time
- Storage
- File descriptors
- Processes
- Threads
- IPC objects
- Network buffers
- Device queues

Elvaris should eventually provide resource limits and accounting.

The system should define behavior when a resource limit is reached.

Possible responses include:

- Returning an error
- Blocking
- Terminating a restricted operation
- Restarting a service
- Applying backpressure
- Invoking an administrator policy

## 20. Authentication and Identity

Elvaris will eventually need a clear identity model.

Identity may include:

- Users
- Groups
- Services
- Applications
- Devices
- Remote clients

The identity system should define:

- Creation
- Authentication
- Authorization
- Ownership
- Delegation
- Revocation
- Recovery
- Credential storage

Authentication mechanisms should not expose secret values through logs or diagnostics.

## 21. Cryptography

Cryptography should be provided through reviewed and maintained implementations rather than custom algorithms.

Potential cryptographic uses include:

- Secure boot
- Disk encryption
- File integrity
- Package signing
- Update verification
- Network security
- User authentication
- Secure storage

Elvaris should document:

- Which algorithms are supported
- Which algorithms are deprecated
- How keys are stored
- How keys are rotated
- How compromised keys are revoked
- How cryptographic failures are handled

## 22. Secure Boot and Startup

The startup chain may eventually support:

- Verified boot components
- Signed kernels
- Signed system services
- Measured boot
- Boot configuration protection
- Recovery environments
- Rollback after failed updates

Secure boot should not prevent developers from using documented development and debugging workflows.

Development builds may use different trust settings from production builds.

## 23. Updates and Package Security

System updates are security-sensitive.

The update system should eventually support:

- Signed packages
- Verified metadata
- Secure transport
- Atomic updates
- Rollback
- Dependency validation
- Version checks
- Recovery after interrupted updates
- Audit records

The system should not install packages solely because they have a particular filename or location.

## 24. Logging and Auditing

Security-relevant events should be observable without exposing unnecessary sensitive information.

Potential events include:

- Authentication failures
- Permission failures
- Privilege changes
- Service crashes
- Driver failures
- Package installation
- Update verification
- Security-policy changes
- Unexpected kernel faults
- Remote administrative access

Logs should define:

- Retention
- Access permissions
- Rotation
- Integrity protection
- Privacy limits
- Failure behavior

## 25. Privacy

Elvaris should avoid unnecessary collection of user information.

The system should:

- Avoid mandatory online accounts
- Avoid unnecessary telemetry
- Document network communication
- Provide user controls
- Minimize sensitive logs
- Protect diagnostic data
- Require consent for optional data collection

Diagnostics should avoid including secrets, credentials, or private file contents.

## 26. Compatibility-Layer Security

Compatibility layers may expect behavior that conflicts with Elvaris security principles.

Compatibility layers should:

- Run with restricted permissions where possible
- Clearly identify their access requirements
- Avoid weakening kernel isolation
- Avoid granting unrestricted device access
- Translate permissions explicitly
- Log security-relevant failures
- Separate unsupported behavior from unsafe behavior

Compatibility must not require the entire operating system to operate with weak security defaults.

## 27. Embedded Security

Embedded deployments may have different security requirements.

The embedded profile may need:

- Secure startup
- Read-only system images
- Device identity
- Signed field updates
- Watchdog recovery
- Restricted debug access
- Physical-tamper assumptions
- Minimal service sets
- Recovery partitions

Each embedded product should define its own threat model.

## 28. Security Testing

Security testing should include:

- Unit tests
- Integration tests
- Fuzz testing
- Invalid-input testing
- Permission tests
- Isolation tests
- Resource-exhaustion tests
- Concurrency tests
- Fault-injection tests
- Update-recovery tests
- Compatibility security tests

Testing should cover both x86-64 and ARM64 where behavior differs.

## 29. Security Review Process

Major security-sensitive changes should receive review before becoming stable.

Security review should ask:

- What new authority is introduced?
- What data crosses a trust boundary?
- What input is untrusted?
- What happens when validation fails?
- What happens if the component is compromised?
- Can the feature run outside the kernel?
- What resources can be exhausted?
- How is the feature tested?
- What information can leak through errors or logs?

## 30. Security Incident Response

Elvaris should eventually define a process for:

- Reporting vulnerabilities
- Classifying severity
- Reproducing issues
- Developing fixes
- Testing fixes
- Publishing advisories
- Delivering updates
- Tracking affected versions
- Communicating workarounds

Security issues should be documented honestly, including affected components and limitations.

## 31. Security Success Criteria

The security model will be considered successful when:

- User processes are isolated.
- Kernel memory is protected.
- System calls validate untrusted input.
- Services have restricted authority.
- Applications cannot access unauthorized resources.
- Updates can be verified.
- Security failures are observable.
- The system has documented trust boundaries.
- Security-sensitive decisions are tested.
- Compatibility features do not silently weaken the whole system.
