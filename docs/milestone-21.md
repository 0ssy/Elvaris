# Milestone 21 — Application Compatibility

## 1. Objective

Enable selected applications from other ecosystems to run on Elvaris without turning Elvaris into a Linux, Windows, or macOS clone.

The compatibility strategy is layered:

`
Native Elvaris applications
        ↓
Portable C and Rust applications
        ↓
POSIX-style portability
        ↓
Selected Linux compatibility
        ↓
Selected Windows compatibility
`

Compatibility must remain optional. A compatibility application must still be subject to:

- Elvaris process isolation
- Elvaris permissions
- Elvaris resource limits
- Elvaris package verification
- Elvaris filesystem policy
- Elvaris recovery and update mechanisms

## Completion target

At the end of Milestone 21, Elvaris should have:

- A stable native application platform
- A documented portability layer
- A selected POSIX compatibility set
- A controlled Linux compatibility path
- A documented Windows compatibility research path
- Compatibility packages with declared permissions
- Application test classifications
- Clear support levels
- No claim of universal application compatibility

## 2. Compatibility Principles

### 2.1 Native Elvaris remains primary
Native applications should receive the best performance, most complete API access, strongest tooling, clearest permission model, and most stable support. Compatibility applications should not define the kernel ABI.

### 2.2 Compatibility is a service layer
Compatibility logic sits above the Elvaris kernel as a runtime/service layer. Foreign system-call behavior must not be added directly into unrelated kernel subsystems without a clear, stable reason.

### 2.3 Compatibility must not weaken security
A compatibility application must not automatically gain administrator privileges, direct hardware access, unrestricted filesystem/network access, access to another application's memory, or permission to bypass package verification/resource limits.

### 2.4 Support must be measurable
Classify applications with: Unsupported, Builds, Starts, Basic functionality, Usable, Well supported, Native integration. Each supported app must have version, target architecture, test suite, known limitations, required permissions, resource requirements, packaging instructions.

## 3. Milestone Phases

21.0 Compatibility architecture and support policy  
21.1 Native API and SDK stabilization  
21.2 Portable C and Rust application support  
21.3 POSIX portability layer  
21.4 Linux runtime foundation  
21.5 Linux application compatibility  
21.6 Compatibility packaging and permissions  
21.7 Graphics, audio, and networking translation  
21.8 Windows compatibility research and foundation  
21.9 Application certification and support matrix  
21.10 Compatibility testing, isolation, and release

Order matters. Windows compatibility comes after native and POSIX foundations are stable.

## Final Milestone 21 Exit Criteria (summary)
All items in the detailed phase exit criteria must be satisfied, all tests pass, known limitations documented, and no universal compatibility claim made.
