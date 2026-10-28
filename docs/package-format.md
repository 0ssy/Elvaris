# Elvaris Package Format

## Overview

The Elvaris package format (`.elvpkg`) is a deterministic binary archive that
contains application or system files, a JSON manifest, checksums, and an
optional cryptographic signature.

## Binary Layout

All integers are little-endian unless otherwise noted.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0      | 6    | magic | ASCII bytes `ELVPKG` |
| 6      | 4    | format_version | Package format version (currently 1) |
| 10     | 4    | architecture | 0 = x86_64, 1 = aarch64 |
| 14     | 4    | manifest_len | Length of the manifest JSON in bytes |
| 18     | N    | manifest | UTF-8 JSON manifest (see manifest section) |
| 18+N   | 4    | file_count | Number of file entries |
| ...    | variable | file_entries | One entry per file |
| ...    | variable | file_contents | Concatenated file contents |
| ...    | 64   | signature | ed25519 signature over all preceding bytes |
| ...    | 1    | signature_algorithm | 0 = none, 1 = ed25519 |

## File Entry

Each file entry has the following layout:

| Size | Field | Description |
|------|-------|-------------|
| 4    | path_len | Length of the file path in bytes |
| N    | path | UTF-8 file path (absolute, e.g. `/bin/hello`) |
| 8    | size | File size in bytes |
| 8    | checksum | First 8 bytes of the SHA-256 hash of the file contents |
| 4    | permissions | Unix-style permission bits |
| 4    | file_type | 0 = regular file, 1 = directory, 2 = executable |

## Manifest

The manifest is a JSON object with the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | yes | Human-readable package name |
| identifier | string | yes | Reverse-domain identifier (e.g. `org.elvaris.hello`) |
| version | string | yes | Semantic version (e.g. `1.0.0`) |
| architecture | string | yes | `x86_64` or `aarch64` |
| entrypoint | string | yes | Absolute path to the executable |
| runtime | string | yes | Required runtime (`none` for native apps) |
| files | array of strings | yes | List of installed file paths |
| permissions | array of strings | no | List of declared permissions |
| dependencies | array of objects | no | List of dependency constraints |
| manifest_version | u32 | yes | Manifest format version (currently 1) |

Each dependency object has:

| Field | Type | Description |
|-------|------|-------------|
| identifier | string | Package identifier |
| constraint | string | Version constraint (`==`, `>=`, `<`, range) |

## Determinism

To produce reproducible packages:

- File entries are sorted lexicographically by path.
- File modification times are set to zero.
- Manifest fields are serialized in a fixed key order.
- Compression is not used (fixed settings).
- No build identifiers or timestamps are embedded.

## Validation Rules

A package is rejected if:

- The magic bytes are not `ELVPKG`.
- The format version is not supported.
- The architecture is not `x86_64` or `aarch64`.
- The manifest is invalid JSON or missing required fields.
- Any file path contains `..` or is not absolute.
- Duplicate file paths exist.
- Any file checksum does not match the actual contents.
- The signature is present but does not verify.
- The package exceeds the maximum size limit.

## Maximum Limits

| Limit | Value |
|-------|-------|
| Maximum package size | 64 MiB |
| Maximum file count | 1024 |
| Maximum path length | 4096 bytes |
| Maximum manifest size | 64 KiB |
| Maximum dependency count | 64 |

## Examples

A minimal unsigned package for a "hello" application:

```text
Magic: ELVPKG
Version: 1
Architecture: x86_64
Manifest: {"name":"hello","identifier":"org.elvaris.hello","version":"1.0.0",
  "architecture":"x86_64","entrypoint":"/bin/hello","runtime":"none",
  "files":["/bin/hello"],"permissions":["console.output"],
  "dependencies":[],"manifest_version":1}
Files: 1 (/bin/hello, 1024 bytes, checksum=..., permissions=0755, type=2)
Contents: [1024 bytes of executable]
Signature: absent (algorithm=0)
```