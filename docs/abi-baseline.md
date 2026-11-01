# ABI Baseline (Milestone 21.1.2)

## x86-64
- Calling: int 0x80, System V style registers (RAX syscall, RDI RSI RDX RCX args)
- Return: RAX (negative = failure, two's complement of error)
- User pointers validated
- Entry behavior documented

## ARM64
- Same logical ABI, different low-level entry mechanism
- Architecture-independent APIs work consistently

## General
- Pointer width matches arch, alignment rules apply
- Error representation consistent
- Endianness as per arch
