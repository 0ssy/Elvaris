# Elvaris SDK Tooling Documentation

## Overview

The Elvaris SDK provides host-side command-line tools for building, packaging,
signing, installing, and running native Elvaris applications. The SDK is
distributed as a single binary, `elvaris-sdk`, with subcommands for each
operation.

## Installation

Build the SDK from the Elvaris source tree:

```sh
cargo build -p elvaris-sdk
```

The binary is placed at `target/debug/elvaris-sdk`. Add it to your `PATH` or
invoke it via the full path.

Set `ELVARIS_HOME` to override the default SDK home directory (default:
`./.elvaris` relative to the current working directory).

## Commands

### elvaris-sdk build

Build a native Elvaris application from source.

```sh
elvaris-sdk build [SOURCE] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--arch` | Target architecture: `x86_64` or `aarch64` | `x86_64` |
| `--profile` | Build profile: `debug`, `release`, `test` | `release` |
| `--out` | Output directory | `<source>/target` |
| `--name` | Binary name (for Cargo projects) | auto-detected |

**Examples:**

```sh
# Build a Rust application for x86-64
elvaris-sdk build ./my-app --arch x86_64 --profile release

# Build for ARM64
elvaris-sdk build ./my-app --arch aarch64
```

The build detects Cargo projects (`Cargo.toml`) and C source directories
automatically. Rust applications are built with `cargo build --target`.

### elvaris-sdk package

Create a `.elvpkg` package from built application files.

```sh
elvaris-sdk package [INPUT] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output package path | required |
| `--name` | Package name | directory name |
| `--identifier` | Reverse-domain identifier | `org.elvaris.<name>` |
| `--version` | Semantic version | `1.0.0` |
| `--arch` | Architecture | `x86_64` |
| `--entrypoint` | Application entrypoint path | `/bin/app` |
| `--permission` | Declare permission (repeatable) | none |
| `--dependency` | Dependency as `id=constraint` (repeatable) | none |

**Examples:**

```sh
# Package a built application
elvaris-sdk package ./my-app/target/x86_64-unknown-none/release/my-app \
    --output my-app.elvpkg \
    --name my-app \
    --identifier org.elvaris.my-app \
    --version 1.0.0 \
    --arch x86_64 \
    --entrypoint /bin/my-app \
    --permission console.output \
    --permission filesystem.read
```

### elvaris-sdk sign

Sign a package with an ed25519 key.

```sh
elvaris-sdk sign [PACKAGE] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output signed package path | `<package>.signed` |
| `--key` | Secret key file (32 bytes raw) | required |
| `--generate-key` | Generate a new key pair | false |

**Examples:**

```sh
# Generate a new key and sign
elvaris-sdk sign my-app.elvpkg --generate-key --output my-app.signed.elvpkg

# Sign with an existing key
elvaris-sdk sign my-app.elvpkg --key ~/.elvaris/keys/development.key
```

### elvaris-sdk verify

Verify a package signature against a public key.

```sh
elvaris-sdk verify [PACKAGE] --key [PUBLIC_KEY]
```

**Example:**

```sh
elvaris-sdk verify my-app.signed.elvpkg --key ~/.elvaris/keys/development.pub
```

### elvaris-sdk install

Install a package into a local repository.

```sh
elvaris-sdk install [PACKAGE] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--repo` | Repository directory | `$ELVARIS_HOME/repos/local` |
| `--key` | Public key for verification | optional (warning if omitted) |

**Example:**

```sh
elvaris-sdk install my-app.signed.elvpkg \
    --repo ./packages/repo \
    --key ./keys/development.pub
```

### elvaris-sdk run

Run a packaged application or binary in QEMU.

```sh
elvaris-sdk run [TARGET] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--arch` | Architecture | `x86_64` |
| `--machine` | QEMU machine type | `q35` |

**Note:** Direct package execution in QEMU requires kernel package support
that is planned for a future milestone. Currently, use `elvaris-sdk update`
to create a bootable system image instead.

### elvaris-sdk repo

Manage a local package repository.

```sh
elvaris-sdk repo [options]
```

| Option | Description |
|--------|-------------|
| `--repo` | Repository directory |
| `--list` | List packages in the repository |

**Example:**

```sh
elvaris-sdk repo --repo ./packages/repo --list
```

### elvaris-sdk update

Stage a system update image.

```sh
elvaris-sdk update [IMAGE] [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--arch` | Architecture | `x86_64` |
| `--verify` | Verify signature before staging | false |
| `--key` | Public key for verification | optional |

**Example:**

```sh
elvaris-sdk update system-image.img --arch x86_64 --verify --key ./keys/production.pub
```

## Application Development

### Creating a Rust Application

1. Create a new Cargo project:
   ```sh
   cargo new my-app
   cd my-app
   ```

2. Add Elvaris target dependencies to `Cargo.toml`:
   ```toml
   [dependencies]
   # Elvaris applications are no_std by default
   ```

3. Build for x86-64:
   ```sh
   elvaris-sdk build . --arch x86_64
   ```

4. Build for ARM64:
   ```sh
   elvaris-sdk build . --arch aarch64
   ```

### Creating a C Application

C application support requires a cross-compiler for the target architecture.
Configure `CC_x86_64_unknown_none` and `CC_aarch64_unknown_none` environment
variables to point to your cross-compilers.

## Package Development Workflow

```text
1. Write application source (Rust or C)
2. elvaris-sdk build --arch x86_64
3. elvaris-sdk package --output app.elvpkg --permission console.output
4. elvaris-sdk sign --generate-key --output app.signed.elvpkg
5. elvaris-sdk verify app.signed.elvpkg --key ~/.elvaris/keys/development.pub
6. elvaris-sdk install app.signed.elvpkg --repo ./packages/repo
7. elvaris-sdk update system.img --arch x86_64 --verify --key ./keys/production.pub
```

## Directory Layout

```
$ELVARIS_HOME/
  packages/     # Built .elvpkg packages
  keys/         # Signing keys (development.pub, production.pub)
  repos/        # Local package repositories (local/index.json)
```

## Package Format

Packages use the `.elvpkg` extension and follow the binary format documented
in `docs/package-format.md`. Key characteristics:

- Magic: `ELVPKG`
- Format version: 1
- Architecture: `x86_64` or `aarch64`
- Manifest: JSON
- Signature: ed25519 (optional)

## Security Policy

- Unsigned packages generate a warning but can be installed for development
- Production installation requires signature verification with a trusted key
- Public keys are stored in `$ELVARIS_HOME/keys/`
- Package paths are validated to prevent traversal attacks
- File ownership is tracked per package

## Build Profiles

| Profile | Optimization | Debug Info | Assertions | Signature Policy |
|---------|-------------|------------|------------|------------------|
| debug | none | full | enabled | development |
| release | speed | minimal | disabled | production |
| test | none | full | enabled | development |

Development builds must not silently receive production trust. Sign a package
with a development key for testing; re-sign with a production key for release.

## Known Limitations

- C cross-compilation requires manual cross-compiler configuration
- Direct package execution in QEMU is not yet implemented
- ARM64 system images require the ARM64 kernel port (Milestone 16)
- Package installation scripts are not supported
- Delta updates are not supported