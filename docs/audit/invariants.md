# Critical Invariants (Milestone 22.2)

- User memory cannot access kernel memory.
- Released frames cannot be released twice.
- A terminated process cannot run.
- A device has at most one active owner.
- A surface cannot access outside its buffer.
- A malformed packet cannot crash the kernel.
- A package cannot escape its staging directory.
- A failed update preserves the last known-good system.
- Resource cleanup on failure is required.
- User pointers validated before dereference.
