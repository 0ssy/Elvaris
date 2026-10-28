# Milestone 18: Graphics and Desktop Foundation

## Objective

Add an optional graphical environment to Elvaris while preserving the working
command-line and server systems. Graphics must be additive: if graphics
initialization fails, the system must continue to boot and operate via the
serial console or text console.

The long-term flow is:

```text
Display hardware
    ↓
Framebuffer/display driver
    ↓
Graphics surface
    ↓
Compositor
    ↓
Window manager
    ↓
Graphical session
    ↓
Desktop applications
```

Milestone 18 does not implement the full flow. It establishes the foundations
and proves that a framebuffer can be displayed on x86-64 in QEMU.

## Initial Target

| Decision | Value |
|----------|-------|
| Architecture | x86-64 first |
| Machine | QEMU (`q35` or `pc` with VGA/QXL framebuffer) |
| Display | bootloader-provided framebuffer |
| Rendering | CPU software rendering |
| Input | existing terminal/input event model |
| GPU acceleration | deferred |
| ARM64 graphics | later validation target |

ARM64 graphics remains a later validation target after the x86-64 framebuffer
path works. No ARM64 graphics code is written in this milestone.

## Graphics Architecture

The graphics stack is layered so each stage can be implemented and tested
independently:

```text
Application
    ↓
Graphics API / IPC (future)
    ↓
Compositor (future)
    ↓
Window manager (future)
    ↓
Graphics subsystem
    ↓
Framebuffer/display driver
    ↓
Display hardware
```

Milestone 18 implements only the bottom two layers (framebuffer driver and
graphics subsystem) plus software rendering primitives. Everything above is
deferred to later phases.

## Display Backend

The bootloader-provided framebuffer is used first. The bootloader (UEFI via
the `bootloader` crate) exposes a framebuffer with a physical address, width,
height, and pixel format. The kernel maps this memory and exposes it to the
graphics subsystem.

### QEMU display configuration

- Machine: QEMU x86-64 `q35` or `pc`
- Video: VGA or QXL framebuffer provided by the bootloader
- Framebuffer: linear pixel buffer at a physical address reported by the
  bootloader's memory map / framebuffer info
- Resolution: bootloader-dependent (typically 640x480 or 800x600); the kernel
  reads the actual dimensions from the framebuffer info rather than hardcoding

### Explicitly not used initially

- GPU drivers
- 3D rendering
- Hardware acceleration
- Window-system protocols (Wayland, X11)
- Multiple monitors
- EDID parsing or display hotplug

## Pixel Format Strategy

The pixel format is taken from the bootloader framebuffer info. Common
bootloader formats:

- RGB (8-8-8)
- BGR (8-8-8)
- RGBA / BGRA (8-8-8-8)

The graphics subsystem stores the format, bytes-per-pixel, and pitch (stride)
reported by the bootloader. All rendering primitives operate in this native
format. A conversion layer is added later if applications need to render in a
different format.

## Rendering Strategy

CPU software rendering initially. The first rendering primitives are:

- Clear the screen (fill with a color)
- Set a pixel
- Draw a filled rectangle
- Copy a surface (blit)
- Render basic text (deferred to Phase 18.8 or later, using a bitmap font)

Rendering is synchronous and single-threaded initially. Clipping and
coordinate validation are included so out-of-bounds draws are rejected
cleanly.

## Surface Ownership

One component owns the framebuffer: the graphics subsystem.

```text
Application
    ↓
Graphics API or IPC (future)
    ↓
Graphics subsystem (owner)
    ↓
Framebuffer
```

Initially the graphics subsystem lives in the kernel (transitional, see
Kernel Boundary below). Applications do not write directly to the framebuffer.
They request drawing operations through the graphics subsystem.

## Process and Security Boundaries

### Kernel boundary

Graphics logic is kept out of the kernel where practical. The kernel provides
only what is necessary:

- Mapping framebuffer memory into kernel address space
- Protecting display memory from unauthorized access
- Handling required device access
- Scheduling graphics-related processes or services

Rendering, surfaces, and composition eventually run in user space.

For the first framebuffer test, a small amount of kernel-side code is
acceptable: a `graphics` module that maps the framebuffer and exposes drawing
primitives. This is documented as transitional and is moved to user space in a
later phase.

### Application access

Applications do not map framebuffer memory directly. They communicate with the
graphics subsystem via IPC or a graphics API (future phases). Unauthorized
attempts to access display memory are rejected by the kernel's memory
protection.

## Failure and Recovery

The command line must remain available if graphics initialization fails.

```text
Graphics failure
    ↓
Log diagnostic (serial console)
    ↓
Return to serial or text-console mode
    ↓
Continue booting or shut down safely
```

A graphics failure must not make the entire operating system unbootable. The
system boots to the serial console even if the framebuffer is missing, the
pixel format is unsupported, or rendering fails.

Graphics initialization is attempted after the serial console is operational,
so diagnostics are always available.

## Initial Scope

- Map the bootloader-provided framebuffer
- Expose framebuffer dimensions, pitch, and pixel format
- Software rendering primitives: clear, pixel, rectangle, blit
- Framebuffer ownership by the graphics subsystem
- Graceful degradation to serial console on failure
- Diagnostics for graphics initialization and rendering errors

## Out of Scope

- GPU drivers
- 3D rendering or hardware acceleration
- Window-system protocols
- Compositor
- Window manager
- Graphical session
- Terminal emulator
- Desktop shell
- Desktop applications
- Multiple monitors
- Display hotplug
- EDID parsing
- Cursor / mouse pointer
- Anti-aliased text or font rendering
- Double buffering or page flipping
- ARM64 graphics (deferred)

## Phase 18.0 Exit Criteria

This planning phase is complete when the following are decided and documented:

- [x] Initial target architecture: x86-64
- [x] QEMU display configuration: bootloader-provided framebuffer
- [x] Framebuffer source: bootloader framebuffer info
- [x] Pixel format strategy: bootloader-native format (RGB/BGR/RGBA/BGRA)
- [x] Software-rendering decision: CPU rendering, primitives first
- [x] Framebuffer ownership: graphics subsystem owns it
- [x] Kernel/user-space boundary: kernel maps memory, rendering transitional in kernel
- [x] Failure and recovery behavior: graphics failure falls back to serial console
- [x] ARM64 graphics timing: deferred, later validation target
- [x] Explicit out-of-scope features: listed above

Once documented, proceed to Phase 18.1 — Framebuffer and Display Output.

---

## Phase 18.1 — Framebuffer and Display Output

### Objective

Make Elvaris display reliable pixels through the bootloader-provided
framebuffer. Read framebuffer information from `BootInfo`, validate it, create
a safe display abstraction, and render a simple test pattern in QEMU.

Expected result:

```text
Bootloader framebuffer
        ↓
Framebuffer validation
        ↓
Display abstraction
        ↓
Clear screen
        ↓
Draw test rectangles
        ↓
Return to serial/test flow
```

### Framebuffer Information

The following fields are read from `BootInfo.framebuffer` (via
`bootloader_api::info::FrameBuffer`):

| Field | Source | Meaning |
|-------|--------|---------|
| Address | `FrameBuffer.buffer_start` | Physical address of pixel buffer |
| Byte length | `FrameBufferInfo.byte_len` | Total buffer size in bytes |
| Width | `FrameBufferInfo.width` | Pixels per row |
| Height | `FrameBufferInfo.height` | Number of rows |
| Stride | `FrameBufferInfo.stride` | Bytes between row starts (may include padding) |
| Bytes per pixel | `FrameBufferInfo.bytes_per_pixel` | Bytes per pixel |
| Pixel format | `FrameBufferInfo.pixel_format` | `Rgb`, `Bgr`, `U8`, or `Unknown` |

The stride is **not** assumed to equal `width × bytes_per_pixel`. Padding
between rows is supported.

### Validation Rules

Before any pixels are written, the framebuffer is validated:

1. Width is nonzero.
2. Height is nonzero.
3. Bytes per pixel is nonzero.
4. Stride is at least `width × bytes_per_pixel`.
5. `stride × (height − 1) + width × bytes_per_pixel` does not overflow.
6. The total needed byte range is within `byte_len`.
7. The pixel format is recognized (`Rgb`, `Bgr`, `U8`, or `Unknown`).

If any check fails, `graphics: initialization failed` is reported via serial
and the kernel falls back to serial mode. The kernel does not panic.

### Supported Pixel Formats

| Format | Channel order | Alpha handling |
|--------|-------------|----------------|
| `Rgb` | R, G, B | Opaque (0xFF) if bpp > 3 |
| `Bgr` | B, G, R | Opaque (0xFF) if bpp > 3 |
| `U8` | Grayscale | N/A |
| `Unknown` | Raw LE bytes | N/A |

The initial renderer ignores alpha blending and writes fully opaque pixels.
No alpha compositing is implemented.

### Display Abstraction

The `Framebuffer` struct (`Kernel/src/graphics.rs`) provides:

- `width`, `height`, `stride`, `bytes_per_pixel`, `pixel_format` accessors
- `clear(color)` — fill the entire screen
- `set_pixel(x, y, color)` — write one pixel, bounds-checked
- `fill_rect(x, y, w, h, color)` — filled rectangle, clipped to bounds
- `report()` — serial diagnostics of framebuffer metadata

### Clipping Rules

Rectangles are **clipped** to the display bounds:

- Rectangles partially on-screen are drawn with only the visible portion.
- Rectangles completely off-screen are accepted as no-ops.
- Zero-width or zero-height rectangles are rejected.
- Coordinates at or beyond width/height are rejected for `set_pixel`.
- Arithmetic overflow in offset calculation is rejected.

### Test Pattern

`render_test_pattern()` draws:

1. Clear screen to dark color (`0x002020`)
2. Red rectangle (`0xFF0000`) in upper-left
3. Green rectangle (`0x00FF00`) in upper-right
4. Blue rectangle (`0x0000FF`) in lower-left
5. White rectangle (`0xFFFFFF`) in lower-right
6. One-pixel white border around the screen

This pattern makes visible: incorrect stride, swapped R/B channels, wrong
pixel size, wrong coordinate origin, row-order mistakes, out-of-bounds writes,
and incorrect clipping.

Serial diagnostics printed:

```text
graphics: framebuffer detected
graphics: width=...
graphics: height=...
graphics: stride=...
graphics: bytes_per_pixel=...
graphics: pixel_format=...
graphics: test pattern rendered
```

### Failure Behavior

| Failure | Serial message | Result |
|---------|---------------|--------|
| No framebuffer | `graphics: unavailable` | Continue without graphics |
| Unsupported format | `graphics: unsupported format` | Continue without graphics |
| Invalid dimensions/stride | `graphics: invalid metadata` | Continue without graphics |
| Memory-access failure | Reported + graphics disabled | Continue without graphics |

A framebuffer failure never corrupts the kernel and never prevents boot.

### Testing Strategy

#### Unit tests (`Kernel/src/graphics.rs`, `#[cfg(test)]`)

- [x] RGB channel order
- [x] BGR channel order
- [x] U8 grayscale average
- [x] Pixel offset calculation
- [x] Stride larger than row width (padding)
- [x] First pixel (0, 0)
- [x] Last valid pixel (width−1, height−1)
- [x] Out-of-bounds pixel rejection
- [x] Rectangle clipping to bounds
- [x] Empty rectangle (zero size) rejected
- [x] Oversized rectangle clipped
- [x] Arithmetic overflow handling
- [x] Byte length calculation
- [x] Off-screen rectangle is no-op

#### Integration test

The QEMU boot test verifies:
- [ ] Kernel detects framebuffer
- [ ] Framebuffer metadata is reported via serial
- [ ] Test pattern rendering completes
- [ ] Serial success marker appears
- [ ] Existing boot test still passes
- [ ] QEMU exits deterministically

Visual inspection supplements automated testing: colors correct, rectangles in
expected locations, no screen shift, no visible corruption.

### Exit Criteria

- [x] Framebuffer information is read from BootInfo
- [x] Framebuffer metadata is validated
- [x] Supported pixel formats are documented
- [x] Pixel offset calculation is tested
- [x] Pixel writes are bounds-safe
- [x] Rectangle drawing is bounds-safe
- [x] Framebuffer abstraction exists (`Kernel/src/graphics.rs`)
- [x] Deterministic test pattern renders
- [x] Serial framebuffer diagnostics appear
- [ ] No-framebuffer fallback works (needs QEMU test without framebuffer)
- [ ] Unsupported-format fallback works (needs QEMU test with bad format)
- [x] Unit tests pass (15/15)
- [ ] QEMU framebuffer test passes
- [x] Existing boot, shell, filesystem, process, security, and network tests pass
- [x] No compositor or window-system code was added
- [x] Phase limitations are documented

### Definition of Done

Phase 18.1 is complete when Elvaris can:

```text
Boot in QEMU
    ↓
Detect and validate the framebuffer
    ↓
Render a visible test pattern
    ↓
Continue reporting through serial output
    ↓
Pass automated tests
    ↓
Fall back safely if graphics are unavailable
```

Rule: First prove that Elvaris can safely control pixels. Do not build a
desktop until the framebuffer foundation is reliable.