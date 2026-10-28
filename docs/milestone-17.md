# Milestone 17: Packaging, Tooling, and Updates

## Objective

Make Elvaris practical for application developers and system administrators.

A developer should be able to:

```text
Build an application
    ↓
Package it
    ↓
Generate metadata
    ↓
Sign the package
    ↓
Install it on Elvaris
    ↓
Run it with declared permissions
    ↓
Upgrade or remove it
```

A system administrator should be able to:

```text
Verify an update
    ↓
Install it safely
    ↓
Recover from an interrupted update
    ↓
Roll back if necessary
```

Milestone 17 is about the native Elvaris ecosystem. It does not include Linux
or Windows compatibility packaging.

## Package Model

A package is a self-describing, cryptographically verifiable unit of
distribution. It contains:

- A manifest describing the package (name, version, architecture, entry point,
  permissions, dependencies).
- Application or system files with checksums.
- An optional cryptographic signature.

Packages are immutable once signed. The package file extension is `.elvpkg`.

## Application Manifest

Every application package declares:

```text
name              - human-readable package name
identifier        - reverse-domain package identifier (org.elvaris.hello)
version           - semantic version (major.minor.patch)
architecture      - x86_64 or aarch64
entrypoint        - absolute path to the executable
runtime           - required runtime (none for native Elvaris apps)
files             - list of installed file paths
permissions       - list of declared permissions
dependencies      - list of package identifiers with version constraints
manifest_version  - manifest format version
```

## Package Layout

A `.elvpkg` file is a deterministic binary archive:

```text
[magic: 6 bytes "ELVPKG"]
[format_version: u32]
[architecture: u32]
[manifest_len: u32]
[manifest: JSON bytes]
[file_count: u32]
[file entries: path, size, checksum, permissions]
[file contents: concatenated]
[signature: 64-byte ed25519 signature]
[signature_algorithm: u8]
```

Files are sorted deterministically by path. Timestamps are zeroed for
reproducibility.

## File Ownership

Each installed file records its owning package in the package database. The
installer rejects:

- Unauthorized overwrite of files owned by another package
- Conflicting package paths
- Removal of files owned by another package

## Permissions

Applications declare required permissions in the manifest. The security model
(from Milestone 14) enforces them at the system-call level. Initial permissions:

```text
console.input
console.output
filesystem.read
filesystem.write
network.client
network.server
process.inspect
device.access
system.shutdown
```

## Dependencies

Dependencies use simple version constraints:

```text
exact version       == 1.0.0
minimum version     >= 1.0.0
compatible range    >= 1.0.0, < 2.0.0
```

No transitive dependency solver is included in the initial release.

## Repository Model

A local repository is a directory containing:

```text
packages/     - .elvpkg files
index.json    - repository metadata with package list, checksums, signatures
```

Repository metadata is signed alongside package signatures.

## Signing Model

- Algorithm: ed25519
- Public key: 32-byte raw public key
- Signature: 64-byte raw signature
- Trust roots: stored in `/etc/elvaris/trust/`
- Development keys: explicitly trusted by the developer
- Production keys: officially signed packages only

## Installation Transactions

Installation follows a transaction-like sequence:

```text
Read package
    ↓
Validate package
    ↓
Check dependencies
    ↓
Check conflicts
    ↓
Stage files
    ↓
Verify staged files
    ↓
Update package database
    ↓
Activate package
```

If any stage fails, staging data is discarded and the existing installation is
left unchanged.

## Update Strategy

A/B inactive-slot update model:

```text
Active slot A    Inactive slot B
                   ↓
               Install update into B
               ↓
               Verify B
               ↓
               Mark B as next boot target
               ↓
               Boot B
               ↓
               Confirm successful startup
               ↓
               Keep B active

If B fails: boot previous known-good slot A
```

## Rollback Strategy

- Boot the previous known-good image
- Identify an incomplete update
- Cancel a staged update
- Restore package database consistency
- Report the failure reason
- Preserve user data where possible
- Prevent repeated boot loops

## Tooling

Host-side SDK tools:

```text
elvaris-build   - build native Elvaris applications (Rust, C)
elvaris-package - create .elvpkg packages
elvaris-install - install packages on Elvaris
elvaris-run     - launch a packaged application
elvaris-sign    - sign packages
elvaris-verify  - verify package signatures
elvaris-repo    - manage local package repository
elvaris-update  - stage and apply system updates
```

## Testing Strategy

Unit tests for manifest parsing, package creation/reading, checksum
validation, signature verification, path validation, and deterministic output.

Integration tests in QEMU for package installation, application launch,
permission enforcement, package removal/upgrade, system image boot, update
staging, activation, rollback, and recovery.

All previous milestone tests must continue to pass.

## Diagnostics

Package and update events:

```text
package: parsed
package: signature verified
package: rejected
package: dependency resolved
package: staged
package: installed
package: removed
package: upgraded
update: received
update: verified
update: staged
update: activated
update: health check passed
update: rollback started
update: rollback completed
update: recovery required
```

Diagnostics must not expose private signing keys, credentials, or sensitive
application data.

## Out of Scope

- Linux binary packages
- Windows installers
- POSIX package compatibility
- GUI software-center application
- Distributed global package repository
- Automatic dependency solving across arbitrary repositories
- Container images
- Language-specific package registries
- Full source-distribution service
- Cloud build infrastructure
- Secure boot hardware integration
- Mandatory online accounts
- Telemetry
- Mobile application stores
- Universal package format compatibility
- Every possible package compression format

## Exit Criteria

Milestone 17 is complete only when every item in Section 23 (Exit Criteria) of
the milestone plan is satisfied, including:

- Package format and manifest documented
- Native Rust and C applications can be built and packaged
- x86-64 and ARM64 applications can be packaged
- Package signatures work
- Package installation, removal, and upgrade work
- File ownership is tracked
- Dependencies and local repository work
- System images can be built and boot on x86-64 and ARM64
- System updates can be staged, verified, activated, and rolled back
- Failed updates leave a bootable system
- Developer and administrator tooling is documented
- All previous milestone tests pass
- Package and update tests execute and pass