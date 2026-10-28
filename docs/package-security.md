# Elvaris Package Security

## Signing Model

### Algorithm

Packages are signed using ed25519 (RFC 8032). No other signature algorithm is
supported in the initial release.

### Key Formats

- Public key: 32-byte raw ed25519 public key
- Signature: 64-byte raw ed25519 signature
- Key identifier: first 8 bytes of the SHA-256 hash of the public key, hex-encoded

### Trust Roots

Trusted public keys are stored in `/etc/elvaris/trust/`:

```text
/etc/elvaris/trust/
  development/    - developer-specified trusted keys
  production/     - officially signed package keys
```

Each key file is named `<key-id>.pub` and contains the 32-byte raw public key.

### Trust Levels

| Level | Description |
|-------|-------------|
| Unsigned development package | No signature; trusted only if explicitly allowed by the developer |
| Locally trusted package | Signed with a key in `/etc/elvaris/trust/development/` |
| Officially signed package | Signed with a key in `/etc/elvaris/trust/production/` |
| Invalid or rejected package | Bad signature, unknown key, or tampered contents |

## Signature Verification Process

Before installation, the verifier:

1. Parses the package and validates the manifest.
2. Computes checksums for every file and compares against recorded values.
3. If a signature is present, looks up the signing key by key identifier.
4. Verifies the ed25519 signature over all bytes preceding the signature.
5. Checks the key against the appropriate trust store.
6. Rejects the package if any step fails.

## Signature Metadata Integrity

Signature metadata cannot be replaced after verification. The signature covers
all package bytes preceding it, so any change to the manifest, file table, or
file contents invalidates the signature.

## Development vs Production Keys

- Development keys: used during development; explicitly trusted by the
  developer; never used for production releases.
- Production keys: used for official Elvaris releases; installed in the
  production trust store by the system image builder.

## Key Rotation and Revocation

Key rotation is supported by adding a new key to the trust store and signing
packages with the new key. Revocation is not implemented in the initial
release; compromised keys are removed from the trust store.

## Path Validation

All package paths are validated before installation:

- Paths must be absolute (starting with `/`).
- Paths must not contain `..` components.
- Paths must not be empty.
- Paths must not exceed the maximum path length.
- Duplicate paths are rejected.

## Package Database Protection

The package database (`/var/lib/elvaris/packages/`) is protected:

- Only the package installer may modify it.
- All modifications are logged.
- Corruption is detected via checksums on the database file.

## Security Requirements

- Package signatures must be verified before installation.
- Trust roots must be protected from unauthorized modification.
- All package paths must be validated to prevent path traversal.
- Unauthorized file replacement must be prevented.
- Package permissions must be enforced at the system-call level.
- The package database must be protected from unauthorized modification.
- Downgrade attacks must be prevented where required (version checks).
- Package installation must be restricted to authorized processes.
- System updates must be verified before activation.
- The previous system must remain available during updates.
- Package installation scripts are not supported in the initial release.
- Security-sensitive package operations must be logged.

## What Is Not Supported

- Arbitrary package installation scripts
- Package scripts running with unrestricted authority
- Secure boot hardware integration
- Full disk encryption