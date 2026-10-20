# Milestone 11: Storage, Filesystem, Program Loading, and the Initial Userland

## Objective

Give the machine a filesystem, load programs from it, and turn the machine
into something a person can talk to: the kernel starts an `init` program, the
`init` program starts a shell, and the shell runs the commands of that person
and reports what they did.

## Scope

In this milestone:

- A block-device interface and a memory-backed device.
- A filesystem interface, and an in-memory filesystem that implements it.
- A program-image format and a loader that starts a program out of a file.
- An `init` program that starts the shell.
- A serial console the shell can read from and write to.
- A shell with eleven built-in commands and one external program.

Not in this milestone:

- Persistent storage (phase 11.7 of the plan). The filesystem lives in the
  memory of the kernel and is lost when the machine stops.
- More than one program at a time. The programs run one after the other.
- Pipes, redirection, background jobs, environment variables, scripting.

## The Block Device

`Kernel/src/block/mod.rs` defines the interface the filesystem talks to:

```text
block_size()      the size of one block in bytes
block_count()     the number of blocks of the device
read(block, buf)  read one block into a buffer of exactly one block
write(block, buf) write one block from a buffer of exactly one block
flush()           make the writes durable
is_ready()        whether the device can be used
identify()        a short description of the device
```

The interface is deliberately narrow. A device that is asked for a block
outside of its range, or for a buffer that does not hold exactly one block,
refuses the request and says why; it never writes outside the buffer it was
given. `MemoryBlockDevice` is the implementation used by the milestone: a
fixed buffer of blocks in the memory of the kernel, with the same rules. The
`block` module is part of the `alloc` feature of the kernel, so it is
compiled into the bare kernel binary and not only into the tests.

## The Filesystem Interface

`Kernel/src/fs/mod.rs` describes what a filesystem has to be able to do. The
interface does not know anything about the shell, about program images or
about the machine it runs on:

| Group | Operations |
|-------|------------|
| Mount | `mount`, `unmount`, `is_mounted` |
| Files | `create_file`, `open`, `read`, `write`, `seek`, `close`, `duplicate`, `delete` |
| Directories | `create_dir`, `read_dir` |
| Metadata | `stat` |

### Paths

`Kernel/src/fs/path.rs` holds the rules of the paths, and every path of the
kernel goes through it: the paths of the system calls, the paths of the
program loader and the paths of the filesystem itself.

- `/` is the separator and the root of the filesystem.
- A path is a sequence of components separated by `/`.
- An empty path is not a path.
- Repeated separators are collapsed: `/bin//sh` is `/bin/sh`.
- `.` is the current directory and never changes the path.
- `..` is the parent directory. At the root it stays at the root: `/..` is `/`.
- A relative path is resolved against the root, because the kernel has no
  per-process current directory yet.
- A component may not be empty, may not hold a NUL byte and may not be longer
  than 255 bytes.
- A path may not be longer than 1024 bytes.

`Path::parse` returns the canonical form of a path: absolute, without an
empty component, and without `.` or `..`. Two paths that name the same object
have the same canonical form.

### Permissions

Every object has an owner and three permissions: read, write and execute.

| Owner | Meaning |
|-------|---------|
| `System` | Only the kernel and the programs it starts may use the object. |
| `User` | Every program of the userland may use the object. |

The kernel runs every program as the system identity, so a `System` object is
usable by the userland, and a `User` object is usable by everybody.

### Errors

The filesystem reports every failure as an `FsError`: `NotFound`,
`AlreadyExists`, `NotADirectory`, `IsADirectory`, `PermissionDenied`,
`InvalidPath`, `NotMounted`, `IoError` and `TooManyHandles`. The system calls
translate them into the negative return values of the userland.

### Handles

A file that is open is named by a handle. A handle belongs to the program
that opened it, is checked before every access, and is closed with `close`.
`duplicate` gives a second handle to the same file. A handle that was closed
or that was never opened is refused, and so is a handle of a file that was
deleted while it was open.

## The In-Memory Filesystem

`Kernel/src/fs/memfs.rs` implements the interface with a tree of nodes in the
memory of the kernel. The tree is the whole filesystem:

```text
/
├── bin/          the programs of the machine
├── dev/          the devices of the machine
├── etc/          the configuration of the machine
├── tmp/          the scratch space of the machine
└── init          the first program of the machine
```

A node is a file or a directory. A file holds its bytes and its permissions;
a directory holds the names of its children and its permissions. The
filesystem counts the files it holds, so the kernel can report how many there
are.

`Kernel/src/storage.rs` owns the filesystem of the machine. It mounts it
during the boot, installs the programs of the machine in it, and hands it to
the code that needs it through `with_filesystem`.

## The Program Image

`Kernel/src/user/image.rs` describes the file the kernel loads a program
from. The format is a fixed header and a block of machine code:

```text
offset  size  field
0       4     magic, "ELVI"
4       4     the version of the format
8       4     the offset of the entry point inside the payload
12      4     the size of the payload in bytes
16      ...   the payload: the machine code of the program
```

The payload is loaded at the base of the user code page, so the entry point
of the program is the base of the code page plus the offset of the entry
point. There is no relocation, no symbol table and no dynamic linking, so an
image is only usable at the address the loader puts it at.

An image is refused when it is too short to hold a header, when it does not
start with the magic, when it has a version the kernel does not know, when
the size of its payload does not fit into the file, when its entry point does
not point inside the payload, or when its payload is larger than the user
code page. The kernel never loads part of an image and never runs an image
whose entry point it could not check.

## The Program Loader

`Kernel/src/user/loader.rs` loads a program out of the filesystem and starts
it. The path of the program is a path of the filesystem, a program without a
separator is looked for in `/bin`, and a program that is not there is
reported as missing.

The loader

1. reads the file of the filesystem,
2. checks the header of the image,
3. makes the code page of the user window writable, copies the payload into
   it, and makes the page read-only again,
4. writes the arguments of the program into the data page, and
5. starts the program with `iretq` in ring 3.

A path that holds a separator is the path of the program. A path without a
separator is looked for in `/bin` first and in the root afterwards, so that
`hello` finds `/bin/hello` and `init` finds `/init`.

The arguments of a program are its path, its arguments and the count of its
arguments, in the order every program expects them:

```text
RDI  the path of the program
RSI  the number of the arguments
RDX  the address of the first argument
RCX  the address of the array of the addresses of the arguments
```

## The User Window

The userland runs in the same window as the user program of the earlier
milestones, and the window is a fixed piece of the address space:

```text
0x0000_0080_0000_0000  user code      1 page   read + execute
0x0000_0080_0000_1000  user data      1 page   read + write, no execute
0x0000_0080_0000_2000  user stack     4 pages  read + write, no execute
0x0000_0080_0000_6000  end of the user address space
```

A program that starts another program does not get an address space of its
own. The kernel saves the window into its heap, lets the child program write
over the window, and writes the window back when the child program ends. The
code page is made writable for the copy and read-only again afterwards, so
that a program cannot change the code of itself or of the program that
started it.

## The Console and the System Calls

The console of the machine is the first serial port. The kernel writes its
diagnostics to it, and the shell reads the commands of the person at the
keyboard from it. The input of the console is a separate path from the
output, so the shell can be tested without a keyboard.

The system calls of the userland are the system calls of the earlier
milestones, plus the ones the filesystem needs. The number of the call is in
`RAX`, the arguments are in `RDI`, `RSI`, `RDX` and `RCX`, and the result is
in `RAX`. A negative result is an error.

| Number | Call | Arguments |
|--------|------|-----------|
| 1 | `write` | the file descriptor, the address of the bytes, the number of the bytes |
| 2 | `exit` | the exit code |
| 3 | `open` | the address of the path, the length of the path, the flags |
| 4 | `read` | the file descriptor, the address of the buffer, the size of the buffer |
| 5 | `getdents` | the address of the path, the length of the path, the address of the buffer, the size of the buffer |
| 6 | `close` | the file descriptor |
| 7 | `stat` | the address of the path, the length of the path, the address of the metadata |
| 8 | `exec` | the address of the path, the length of the path, the address of the arguments, the length of the arguments |
| 9 | `meminfo` | — |
| 10 | `procs` | — |
| 11 | `shutdown` | — |
| 12 | `reboot` | — |
| 13 | `format_number` | the value, the address of the buffer, the size of the buffer |

The file descriptor `0` is the console input, and the file descriptor `1` and
`2` are the console output. Every other file descriptor is a handle of the
filesystem.

Every address a program hands to the kernel is untrusted input. A pointer is
only usable when it is canonical, when the whole range it describes lies
inside the user address space, and when every page of the range is mapped,
accessible from user mode, and writable when the call has to write to it.
A program that hands a bad pointer to the kernel gets an error, not a fault
of the kernel.

## The Shell

`Kernel/src/user/programs/shell.rs` is the shell of the machine. It reads a
line from the console, splits it into the command and its arguments, and
runs the command. The line is parsed in the shell itself: the first word is
the command, the rest of the line is the argument of the command.

The built-in commands are:

| Command | What it does |
|---------|--------------|
| `help` | shows the list of the commands |
| `pwd` | shows the current directory |
| `ls [directory]` | lists the entries of a directory, the root when there is no argument |
| `cat <file>` | shows the content of a file |
| `echo <text>` | shows its argument |
| `ps` | shows the processes of the machine |
| `mem` | shows the memory of the machine |
| `clear` | clears the console |
| `shutdown` | stops the machine |
| `reboot` | restarts the machine |
| `exit` | stops the shell |

Every other command is a program of the filesystem. The shell looks for the
name in `/bin`, starts the program, waits for it, and reports the exit
status of the program. A command that names no program is reported as
unknown.

## The `init` Program

`Kernel/src/user/programs/init.rs` is the first program of the machine. The
kernel loads it out of the filesystem, starts it as a user program, and it
starts the shell with `exec`. A shell that cannot be started is reported on
the console, and the `init` program ends with the exit code 1. When the
shell ends, the `init` program ends with the status of the shell, and the
kernel stops the machine.

## Testing

- The unit tests of the kernel cover the block device, the paths, the
  in-memory filesystem, the image format and the userland helpers.
- `Kernel/tests/userland.rs` boots the machine in QEMU, drives the shell
  through its serial console, and checks the answers of the built-in
  commands, the program the shell starts out of the filesystem, the exit
  status it reports, the errors it prints, and the exit code of the machine.
- The other integration tests (`boot`, `user`, `heap`) use the same harness.

The serial console of the guest is a TCP socket, and the test is the client
of that socket. The test waits for the prompt of the shell before it sends
the next command, so the answer of a command is never mixed with the answer
of the next one, and it tells the shell to stop at the end, so that the
machine ends the boot on its own.

## Limitations

- The filesystem lives in the memory of the kernel. Nothing survives the
  machine; phase 11.7 of the plan is the one that changes that.
- The programs run one after the other. There is no scheduler for user
  programs, and a program that starts another program waits for it.
- The programs share one window of the address space, and the kernel saves
  the window in its heap while a child program runs. A program that needs
  more memory than the window holds cannot be run.
- The console is the serial port. There is no keyboard driver, and the
  person at the keyboard types into the serial line.
- The shell knows one argument per command, has no quoting and no pipes.
- The image format has no relocation, so a program only runs at the address
  the loader puts it at.
