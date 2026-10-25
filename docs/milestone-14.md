# Milestone 14 — Security and Identity Foundation

## Objective

Implement and verify the security and identity foundation for Elvaris: process identities, users and groups, file ownership and permissions, privilege model, authorization checks, protected resources, process/device permissions, security-related system calls, security errors, security logging, with comprehensive tests.

## Scope

This milestone implements:
- User identity representation
- Group identity representation  
- Process credentials (uid, gid, supplementary groups, privilege state)
- Initial user and group database
- File owner and group metadata with owner/group/other permission classes
- Read/write/execute permissions
- Permission-checking rules with defined evaluation order
- Privileged identity/capabilities
- Protected system resources
- Permission-denied errors with specific error types
- Process identity inheritance
- Identity-aware system calls
- Security event diagnostics/logging
- Unit and integration tests

## 1. Security Model Overview

The authorization flow is:
```
Process identity
      ↓
Authorization check
      ↓
Resource ownership and permissions
      ↓
Allow or deny operation
```

A normal process must not be able to access resources merely because it knows their path or identifier.

### 1.1 Process Credentials

Every process has credentials containing:
- User identity (UID)
- Primary group (GID)
- Supplementary groups (set of GIDs)
- Privilege state (privileged/unprivileged)

### 1.2 Resource Metadata

Every protected resource has:
- Owner user (UID)
- Owner group (GID)
- Permission bits (owner/group/other with read/write/execute)
- Resource type

Initial resource types: Files, Directories, Processes, Console, Devices, IPC objects

## 2. User and Group Identities

### 2.1 Identity Format
- User identifiers (UID) and group identifiers (GID) are numeric values (u64)
- Identity comparison is by numeric value equality
- Identities are fixed at build time for this milestone (static identity database)
- Identity values have defined ranges and reserved values

### 2.2 Reserved Identifiers
- UID 0 / GID 0: Administrative/root identity (system administrator)
- UID 1000 / GID 1000: Normal user identity (primary user)
- UID 65534 / GID 65534: Nobody/nogroup (reserved for restricted services)
- Other UIDs/GIDs in range 100-999 may be used for system services

### 2.3 Invalid Identifiers
- Any UID/GID outside defined valid ranges is considered invalid
- Identity operations with invalid identities return appropriate errors

### 2.4 Identity Lifetime
- Identities exist for the lifetime of the system (static)
- No dynamic identity creation/deletion in this milestone

## 3. Initial Users and Groups

### 3.1 User Database
Static users defined:
- **root/admin (UID 0)**: System administrator, privileged
- **user (UID 1000)**: Ordinary user, unprivileged  
- **service (UID 65534)**: System service identity, restricted/unprivileged

### 3.2 Group Database
Static groups defined:
- **root/admin (GID 0)**: Administrative group
- **users (GID 1000)**: Primary group for normal users
- **nogroup (GID 65534)**: Group for restricted services

### 3.3 Initial Process Identities
- `init` process (PID 1): runs as privileged root (UID 0, GID 0)
- Shell process: starts as ordinary user (UID 1000, GID 1000) by default
- System files owned by root (UID 0, GID 0)
- User files owned by user (UID 1000, GID 1000)
- Not every process is privileged for security testing

## 4. Privilege Model

### 4.1 Privilege States
Two states:
- **Privileged**: Has administrative capabilities
- **Unprivileged**: Normal user, subject to all permission checks

### 4.2 Privilege Representation
Privilege is determined by:
1. Administrative identity (UID 0 / GID 0) - root is privileged
2. Explicit privilege flag in process credentials
3. Combination of identity and privileges

For this milestone: root (UID 0) is privileged by default.

### 4.3 Privileged Operations (require authorization/privilege)
- Changing file ownership (`chown` equivalent)
- Changing permissions on another user's resource
- Mounting/unmounting filesystems
- Managing other processes (terminate/inspect restricted processes)
- Accessing protected devices
- Changing system identity configuration
- Shutting down or rebooting
- Installing system packages/binaries
- Modifying identity database
- Accessing kernel memory/configuration

### 4.4 Privilege Rules
- Privilege is not automatically granted
- A process cannot grant itself privilege
- Privilege elevation requires explicit authorization
- Administrative operations must be explicitly identified and checked

## 5. Permission Model

### 5.1 Permission Classes
Three classes in evaluation order:
1. **Owner** - permissions for the file/directory owner
2. **Group** - permissions for users in the file's group
3. **Other** - permissions for everyone else

### 5.2 Permission Bits
Three basic permissions:
- **Read (r)**: Read file contents or list directory
- **Write (w)**: Modify file contents or create/delete/rename in directory
- **Execute (x)**: Execute file as program or traverse/search directory

### 5.3 Permission Representation
Permissions stored as:
```rust
struct Permissions {
    owner_uid: u64,
    owner_gid: u64,
    owner_r: bool, owner_w: bool, owner_x: bool,
    group_r: bool, group_w: bool, group_x: bool,
    other_r: bool, other_w: bool, other_x: bool,
}
```

### 5.4 Permission Evaluation Order
1. If process is privileged (root/admin), allow operation (administrative override) - but this is logged
2. If process UID matches resource owner UID, check owner permissions
3. Else if process GID matches resource owner GID or process is in supplementary groups matching owner GID, check group permissions
4. Else check other permissions
5. If no applicable class allows the operation, deny

**Important**: Do not fall through - if owner matches, only owner class is evaluated.

### 5.5 Directory Permissions
- **Read**: List directory entries (read directory contents)
- **Write**: Create, delete, or rename entries in the directory
- **Execute**: Traverse/look up entries inside the directory (search/access files within)
- Execute permission on directories is required to access files inside them

## 6. Process Credentials and Inheritance

### 6.1 Process Creation
- Every process gets credentials assigned at creation
- Credentials stored in process structure

### 6.2 Credential Inheritance
- A newly created process inherits the creator's credentials (uid, gid, supplementary groups, privilege state)
- Parent-child relationship is tracked

### 6.3 Credential Changes
- A process cannot change its own identity/credentials unless it has required privilege
- Changing credentials requires authorization
- No setuid/setgid executables in this milestone

### 6.4 Process Lifecycle
- Credentials persist for process lifetime
- On process exit, credentials are cleaned up with the process

## 7. File Ownership Integration

### 7.1 Ownership Assignment
- Newly created files/directories owned by creating process's UID and primary GID
- Ownership is stored in filesystem metadata

### 7.2 Ownership Rules
- Files created by process: owner = process.uid, group = process.gid
- Directories created by process: same ownership rules
- Program images installed into filesystem: owned by root (UID 0) by default
- System files: owned by root
- User files: owned by the creating user

### 7.3 Ownership Changes
- Changing ownership requires privilege (or being owner in some models, but we require privilege)
- Only privileged processes can chown files

## 8. Authorization Boundary

### 8.1 Authorization Check Flow
Every protected operation must pass through authorization:
```text
request
  ↓
identify caller (get process credentials)
  ↓
locate resource
  ↓
authorize operation (check permissions/privilege)
  ↓
perform operation (only if authorized)
```

### 8.2 Authorization Function
Conceptually: `authorize(caller_creds, resource, operation) -> Result<(), AuthError>`

Authorization is separate from resource operation implementation.

## 9. Protected Resources

### 9.1 Protected Resources List
- Kernel memory / kernel structures
- Other processes' address spaces and private data
- System configuration files (/etc/*)
- Identity database
- System binaries (/bin/* owned by root)
- Device interfaces (/dev/*)
- Shutdown and reboot controls
- Process-management operations (kill/inspect other processes)
- Console configuration

### 9.2 Access Policy
**Ordinary user (UID 1000, unprivileged):**
- Own files: according to permissions
- Public files (readable by other): according to permissions
- Other users' private files: denied
- Kernel memory: denied
- Device management: denied
- Process termination of others: restricted/denied
- Shutdown/reboot: denied
- System config: denied

**Administrator (UID 0, privileged):**
- May perform documented administrative operations
- Subject to explicit checks (privilege not unlimited without audit)

## 10. Process Permissions

### 10.1 Process Operations Requiring Authorization
- Inspecting another process's details/metadata
- Terminating another process (kill)
- Changing another process's identity/credentials
- Waiting for an unrelated process
- Reading process memory/state of others
- Modifying process state of others

### 10.2 Process Permission Rules
- A process can always manage its own children in normal cases
- A process cannot control arbitrary other processes just by PID
- Process owner/creator may have some authority over child processes
- Terminating/killing other users' processes requires privilege
- Reading sensitive process info of others requires privilege or ownership

## 11. Device and Console Permissions

### 11.1 Console
- **Read console input**: Processes need permission to read from console/stdin
- **Write console output**: Generally allowed for output, but controlled
- **Configure console**: Requires privilege
- Console may be shared but access is controlled

### 11.2 Devices
- Direct hardware port access is forbidden to user programs - must use kernel interfaces
- Device files in /dev have ownership/permissions
- Accessing protected devices requires appropriate permissions or privilege
- Ordinary users cannot access raw device interfaces

### 11.3 System Control
- Shutdown requires privilege
- Reboot requires privilege
- Power management requires privilege

## 12. Security-Related System Calls

New system calls for security/identity:
- `getuid()` - get current user ID
- `getgid()` - get current group ID
- `geteuid()` / privilege info
- `getpid()` / `getppid()` - process identification
- `get_process_creds(pid)` - get process credentials (privilege required for others)
- `check_access(path, mode)` - check access permissions
- `chmod(path, mode)` - change permissions (requires ownership/privilege)
- `chown(path, uid, gid)` - change ownership (requires privilege)
- `kill(pid, signal)` - send signal/terminate (with permission checks)

All system calls must:
- Validate arguments and pointers
- Identify caller
- Perform authorization checks
- Return appropriate errors
- Log security events where required

## 13. Error Model

### 13.1 Security-Related Errors
- `InvalidIdentity` - invalid UID/GID
- `InvalidResource` - resource doesn't exist or is invalid
- `InvalidPermission` - invalid permission bits/mode
- `PermissionDenied` - not authorized for operation
- `OperationNotSupported` - operation not implemented
- `NotOwner` - not the owner of resource
- `InsufficientPrivilege` - needs privilege but not privileged
- `ResourceUnavailable` - resource not available
- `BadCredential` - bad credential state

### 13.2 Error Reporting Rules
- Do not report every failure as "not found" - be specific where appropriate
- Avoid leaking protected resource details to unauthorized callers
- Return generic errors when information disclosure is a concern
- All errors are distinct and testable

## 14. Security Logging

### 14.1 Events to Log
- Permission denied
- Privilege-required operation attempted
- Identity change attempt
- Unauthorized process access
- Unauthorized device access
- Protected-file modification attempt
- Shutdown or reboot request
- Security-policy configuration failure
- Privilege escalation attempt
- Credential changes

### 14.2 Log Entry Contents
Safe information only:
```text
event_type
requesting_process_id
requesting_user_id (uid)
resource_type
operation
result (allowed/denied)
timestamp (if available)
```

**Never log:** passwords, private file contents, raw credentials, sensitive data

### 14.3 Log Access
Security logs are observable through diagnostics but protected from unauthorized modification.

## 15. Testing Strategy

### 15.1 Identity Tests
- [x] Valid user identity
- [x] Invalid user identity
- [x] Valid group identity
- [x] Invalid group identity
- [x] Credential inheritance
- [x] Restricted identity changes
- [x] Privileged identity behavior

### 15.2 File Permission Tests
- [x] Owner read allowed
- [x] Owner write allowed
- [x] Owner execute allowed
- [x] Group read allowed
- [x] Group write denied (if not in group with write)
- [x] Other read allowed
- [x] Other write denied
- [x] No permission denied
- [x] Permission changes require authorization
- [x] Ownership changes require authorization

### 15.3 Directory Tests
- [x] Read allows listing
- [x] Write allows creation
- [x] Write allows deletion
- [x] Execute allows traversal
- [x] Missing execute permission blocks traversal
- [x] Protected directory cannot be modified

### 15.4 Process Tests
- [x] Process credentials are assigned
- [x] Child inherits expected credentials
- [x] Ordinary process cannot inspect restricted process data
- [x] Ordinary process cannot terminate unrelated protected process
- [x] Authorized parent can wait for its child
- [x] Unauthorized process wait is rejected

### 15.5 Device and Privilege Tests
- [x] Ordinary user cannot access protected device
- [x] Privileged process can perform allowed device operation
- [x] Ordinary user cannot shut down the system
- [x] Authorized shutdown succeeds
- [x] Direct hardware access remains unavailable

### 15.6 Integration Test Scenario
Boot sequence:
```text
Boot Elvaris
    ↓
Start privileged init (UID 0)
    ↓
Start ordinary-user shell (UID 1000)
    ↓
Create a user-owned file
    ↓
Read the file successfully
    ↓
Attempt to read a protected file
    ↓
Receive permission denied
    ↓
Attempt to change unauthorized permissions
    ↓
Receive permission denied
    ↓
Attempt to access a protected device
    ↓
Receive permission denied
    ↓
Run an authorized administrative operation (as root)
    ↓
Operation succeeds
    ↓
Verify security events were recorded safely
    ↓
Shutdown follows the defined privilege rule
```

## 16. Security Invariants

1. Every process has valid credentials.
2. Credentials cannot be forged by user memory.
3. Every protected resource has an owner or explicit system policy.
4. Permission checks occur before protected operations.
5. A denied operation does not partially modify the resource.
6. Ordinary processes cannot access kernel memory.
7. Ordinary processes cannot directly access hardware.
8. Identity changes require authorization.
9. Process IDs are not authorization by themselves.
10. Permission failures do not corrupt resource state.
11. Security logs do not contain secrets.
12. Administrative operations are explicitly identified.
13. A process cannot grant itself privilege.
14. User pointers are validated before security decisions use their contents.

## 17. Implementation Details

### 17.1 Files Modified/Created
- `Kernel/src/security/mod.rs` (new) - security module, authz, logging
- `Kernel/src/security/identity.rs` (new) - identity types and database
- `Kernel/src/security/authz.rs` (new) - authorization logic
- `Kernel/src/security/error.rs` (new) - security errors
- `Kernel/src/security/log.rs` (new) - security logging
- `Kernel/src/process/process.rs` - extend with credentials (uid/gid/supplementary/privilege)
- `Kernel/src/process/mod.rs` - update process creation/inheritance
- `Kernel/src/fs/mod.rs` - extend permission model to owner/group/other
- `Kernel/src/fs/memfs.rs` - implement new permission checks
- `Kernel/src/user/syscall.rs` - add security syscalls and authz checks
- `Kernel/src/storage.rs` - identity integration
- `Kernel/src/lib.rs` / `Kernel/src/main.rs` - module registration
- `Kernel/tests/security.rs` (new) - security tests

### 17.2 Design Decisions
- Static identity database for simplicity
- Root (UID 0) has administrative privilege
- Owner/group/other permission model (Unix-like)
- Permission evaluation: owner matches takes precedence, no fallthrough
- Directory execute required for traversal
- Privilege checked before permission bits for admin override
- Security logs via serial output with safe content only
- All authorization happens at kernel boundary

## 18. Out of Scope
- Password authentication
- Remote authentication
- Full PAM-like framework
- Secure boot, disk encryption
- Complete capability redesign
- Mandatory access control (MAC) frameworks
- Sandboxing profiles, containers
- SELinux-style policy language
- Windows security compatibility
- POSIX ACLs, extended attributes
- Set-user-ID executables
- Multi-factor authentication
- Audit-log persistence guarantees
- Complete package-signing infrastructure

## 19. Exit Criteria

- [x] User and group identities are defined
- [x] Process credentials are defined (uid, gid, supplementary groups, privilege)
- [x] Credential inheritance works
- [x] Privilege rules are documented and implemented
- [x] File ownership is implemented (uid/gid)
- [x] Basic file permissions are enforced (owner/group/other rwx)
- [x] Directory permissions are enforced (read/write/execute semantics)
- [x] Protected resources are defined
- [x] Authorization occurs before protected operations
- [x] Unauthorized access is rejected
- [x] Process-management permissions are enforced
- [x] Device and console permissions are defined
- [x] Administrative operations require authorization
- [x] Security-related errors are defined
- [x] Security events are observable (logged)
- [x] Identity and permission unit tests pass
- [x] Unauthorized-access integration tests pass
- [x] Existing boot, terminal, filesystem, process, and scheduler tests pass
- [x] No security test depends only on log text
- [x] Limitations are documented

## 20. Limitations

- Static identity database only (no useradd/usermod)
- No password authentication
- No setuid/setgid binaries
- No ACLs or extended attributes
- Simple privilege model (root/non-root)
- Security logs go to serial only, not persisted to disk with guarantees
- No capability-based fine-grained permissions beyond rwx
- Group membership limited to primary group for now (supplementary groups tracked but group permission checks use primary group primarily)

## 21. Verification

The complete scenario passes:
1. Boot Elvaris
2. Start privileged init (UID 0)
3. Start ordinary-user shell (UID 1000)
4. Create a user-owned file
5. Read the file successfully
6. Attempt to read a protected file → Permission denied
7. Attempt to change unauthorized permissions → Permission denied
8. Attempt to access a protected device → Permission denied
9. Run an authorized administrative operation as root → Succeeds
10. Verify security events were recorded safely
11. Shutdown follows the defined privilege rule
