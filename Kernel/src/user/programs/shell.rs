//! The shell of the machine.
//!
//! The shell is the program that talks to the person at the console: it
//! reads a line, decides what the line asks for, and does it. The built-in
//! commands are part of the shell, and every other command is a program of
//! the filesystem that the shell starts with the `exec` system call.
//!
//! # The commands
//!
//! | Command | What it does |
//! |---------|--------------|
//! | `help` | list the commands |
//! | `pwd` | print the current directory |
//! | `ls` | list a directory |
//! | `cat` | print a file |
//! | `echo` | print the arguments |
//! | `ps` | list the processes |
//! | `mem` | report the memory of the machine |
//! | `clear` | clear the screen |
//! | `shutdown` | stop the machine |
//! | `reboot` | stop the machine |
//! | `exit` | end the shell |
//!
//! # The limits
//!
//! - There is no quoting, no redirection and no pipeline: the arguments of a
//!   command are the words that follow it.
//! - A command that is not built in is looked up in the filesystem: a name
//!   without a separator is looked up in `/bin` first.
//! - The shell runs one program at a time and waits for it.
//!
//! # The stack
//!
//! The shell keeps its buffers on the stack, which is the only memory it
//! owns:
//!
//! ```text
//!  0x0000  the line the console reads        1024 bytes
//!  0x0400  the entries of a directory        2048 bytes
//!  0x0C00  the content of a file              512 bytes
//!  0x0E00  the number of an exit status        32 bytes
//! ```

use core::arch::global_asm;

/// The size of the buffer the shell reads a line into.
pub const LINE_BUFFER_SIZE: usize = 1024;

/// The size of the buffer the shell reads the entries of a directory into.
pub const DIRENT_BUFFER_SIZE: usize = 2048;

/// The size of the buffer the shell reads the content of a file into.
pub const FILE_BUFFER_SIZE: usize = 512;

/// The size of the buffer the shell renders a number into.
pub const NUMBER_BUFFER_SIZE: usize = 32;

global_asm!(
    ".section .user_shell_text,\"ax\"\n",
    ".globl elvaris_shell_program\n",
    "elvaris_shell_program:\n",
    // The kernel enters the program with the number of the arguments in RDI
    // and the table of the arguments in RSI. The shell reads its commands
    // from the console, so it needs neither, but it must not trust the
    // content of the other registers.
    "    sub rsp, 4096\n",
    "    and rsp, -16\n",
    "    mov r12, rsp\n",
    // The greeting of the shell.
    "    lea rsi, [rip + .Ltext_greeting]\n",
    "    call elvaris_shell_puts\n",
    ".Lshell_loop:\n",
    // The prompt.
    "    mov eax, 1\n",
    "    mov edi, 1\n",
    "    lea rsi, [rip + .Ltext_prompt]\n",
    "    lea rdx, [rip + .Ltext_prompt_end]\n",
    "    sub rdx, rsi\n",
    "    int 0x80\n",
    // read(0, buffer, 1023): the console reads a whole line, and the shell
    // leaves room for the byte that ends it.
    "    mov eax, 4\n",
    "    xor edi, edi\n",
    "    mov rsi, r12\n",
    "    mov edx, 1023\n",
    "    int 0x80\n",
    "    cmp rax, 0\n",
    "    jle .Lshell_exit\n",
    "    mov r13, rax\n",
    // ---- the command word ----
    // The word is the first word of the line. The spaces in front of it are
    // skipped.
    "    xor r15, r15\n",
    ".Lshell_skip:\n",
    "    cmp r15, r13\n",
    "    jge .Lshell_loop\n",
    "    movzx eax, byte ptr [r12 + r15]\n",
    "    cmp al, 32\n",
    "    je .Lshell_skip_next\n",
    "    cmp al, 9\n",
    "    je .Lshell_skip_next\n",
    "    jmp .Lshell_word\n",
    ".Lshell_skip_next:\n",
    "    inc r15\n",
    "    jmp .Lshell_skip\n",
    // The word ends at the first space or at the end of the line.
    ".Lshell_word:\n",
    "    mov r14, r15\n",
    ".Lshell_word_end:\n",
    "    cmp r15, r13\n",
    "    jge .Lshell_word_done\n",
    "    movzx eax, byte ptr [r12 + r15]\n",
    "    cmp al, 32\n",
    "    je .Lshell_word_done\n",
    "    cmp al, 9\n",
    "    je .Lshell_word_done\n",
    "    inc r15\n",
    "    jmp .Lshell_word_end\n",
    ".Lshell_word_done:\n",
    // The length of the word: the word is between r14 and r15.
    "    mov rax, r15\n",
    "    sub rax, r14\n",
    // The word ends with a zero byte, so that it can be compared with a
    // name. The zero byte takes the place of the space that follows the
    // word, so that the arguments keep their first byte.
    "    cmp r15, r13\n",
    "    jge .Lshell_word_end_of_line\n",
    "    mov byte ptr [r12 + r15], 0\n",
    "    inc r15\n",
    "    jmp .Lshell_word_ready\n",
    ".Lshell_word_end_of_line:\n",
    // The word is the last thing in the line: the zero byte goes behind it.
    "    mov byte ptr [r12 + r15], 0\n",
    ".Lshell_word_ready:\n",
    "    mov rdi, r15\n",
    "    mov r15, rax\n",
    // ---- the arguments ----
    // The arguments are the words that follow the command word.
    "    lea rbx, [r12 + rdi]\n",
    "    mov rcx, rdi\n",
    ".Lshell_args_skip:\n",
    "    cmp rcx, r13\n",
    "    jge .Lshell_args_done\n",
    "    movzx eax, byte ptr [r12 + rcx]\n",
    "    cmp al, 32\n",
    "    je .Lshell_args_next\n",
    "    cmp al, 9\n",
    "    je .Lshell_args_next\n",
    "    jmp .Lshell_args_done\n",
    ".Lshell_args_next:\n",
    "    inc rcx\n",
    "    lea rbx, [r12 + rcx]\n",
    "    jmp .Lshell_args_skip\n",
    ".Lshell_args_done:\n",
    // The length of the arguments is the rest of the line: the number of
    // the bytes between the end of the command word and the end of the line.
    "    mov rbp, r13\n",
    "    sub rbp, rcx\n",
    // The spaces at the end of the line are not part of the arguments.
    ".Lshell_args_trim:\n",
    "    cmp rbp, 0\n",
    "    je .Lshell_args_trimmed\n",
    "    mov rax, rbp\n",
    "    dec rax\n",
    "    movzx eax, byte ptr [rbx + rax]\n",
    "    cmp al, 32\n",
    "    je .Lshell_args_trim_next\n",
    "    cmp al, 9\n",
    "    je .Lshell_args_trim_next\n",
    "    jmp .Lshell_args_trimmed\n",
    ".Lshell_args_trim_next:\n",
    "    dec rbp\n",
    "    jmp .Lshell_args_trim\n",
    ".Lshell_args_trimmed:\n",
    // ---- the built-in commands ----
    // help
    "    lea rsi, [rip + .Lname_help]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_pwd\n",
    "    lea rsi, [rip + .Ltext_help]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_pwd:\n",
    // pwd
    "    lea rsi, [rip + .Lname_pwd]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_ls\n",
    "    lea rsi, [rip + .Ltext_root]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_ls:\n",
    // ls
    "    lea rsi, [rip + .Lname_ls]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_cat\n",
    "    jmp .Lshell_do_ls\n",
    ".Lshell_next_cat:\n",
    // cat
    "    lea rsi, [rip + .Lname_cat]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_echo\n",
    "    jmp .Lshell_do_cat\n",
    ".Lshell_next_echo:\n",
    // echo
    "    lea rsi, [rip + .Lname_echo]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_ps\n",
    "    lea rsi, [rbx]\n",
    "    mov rdx, rbp\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    "    lea rsi, [rip + .Ltext_newline]\n",
    "    mov edx, 1\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_ps:\n",
    // ps
    "    lea rsi, [rip + .Lname_ps]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_mem\n",
    "    mov eax, 10\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_mem:\n",
    // mem
    "    lea rsi, [rip + .Lname_mem]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_clear\n",
    "    mov eax, 9\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_clear:\n",
    // clear
    "    lea rsi, [rip + .Lname_clear]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_shutdown\n",
    "    lea rsi, [rip + .Ltext_clear]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_shutdown:\n",
    // shutdown
    "    lea rsi, [rip + .Lname_shutdown]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_reboot\n",
    "    mov eax, 11\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_reboot:\n",
    // reboot
    "    lea rsi, [rip + .Lname_reboot]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_next_exit\n",
    "    mov eax, 12\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_next_exit:\n",
    // exit
    "    lea rsi, [rip + .Lname_exit]\n",
    "    lea rdi, [r12 + r14]\n",
    "    call elvaris_shell_strcmp\n",
    "    test rax, rax\n",
    "    jnz .Lshell_external\n",
    "    mov eax, 2\n",
    "    xor edi, edi\n",
    "    int 0x80\n",
    "    ud2\n",
    // ---- a program of the filesystem ----
    // The command is not built in: the shell starts the program of the
    // filesystem that has that name.
    ".Lshell_external:\n",
    "    lea rdi, [r12 + r14]\n",
    "    mov rsi, r15\n",
    "    mov rdx, rbx\n",
    "    mov rcx, rbp\n",
    "    mov eax, 8\n",
    "    int 0x80\n",
    "    mov r15, rax\n",
    "    cmp r15, 0\n",
    "    jl .Lshell_external_failed\n",
    "    lea rsi, [rip + .Ltext_status]\n",
    "    call elvaris_shell_puts\n",
    "    mov edi, r15d\n",
    "    lea rsi, [r12 + 3584]\n",
    "    mov edx, 32\n",
    "    mov eax, 13\n",
    "    int 0x80\n",
    "    cmp rax, 0\n",
    "    jle .Lshell_status_end\n",
    "    mov rdx, rax\n",
    "    lea rsi, [r12 + 3584]\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    ".Lshell_status_end:\n",
    "    lea rsi, [rip + .Ltext_newline]\n",
    "    mov edx, 1\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_external_failed:\n",
    "    lea rsi, [rip + .Ltext_not_found]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    // ---- the end of the shell ----
    ".Lshell_exit:\n",
    "    mov eax, 2\n",
    "    xor edi, edi\n",
    "    int 0x80\n",
    "    ud2\n",
    // ---- ls ----
    // The directory is the first argument, or the root when there is none.
    ".Lshell_do_ls:\n",
    // The length of the arguments tells whether there is a directory at all.
    "    cmp rbp, 0\n",
    "    je .Lshell_ls_root\n",
    "    mov r8, rbx\n",
    "    mov r9, rbp\n",
    "    jmp .Lshell_ls_open\n",
    ".Lshell_ls_root:\n",
    "    lea r8, [rip + .Ltext_root]\n",
    "    mov r9, 1\n",
    ".Lshell_ls_open:\n",
    // getdents(path, path_length, buffer, 2048): the kernel writes the
    // entries of the directory into the buffer of the shell.
    "    mov rdi, r8\n",
    "    mov rsi, r9\n",
    "    lea rdx, [r12 + 1024]\n",
    "    mov ecx, 2048\n",
    "    mov eax, 5\n",
    "    int 0x80\n",
    "    cmp rax, 0\n",
    "    jl .Lshell_ls_failed\n",
    "    mov r15, rax\n",
    "    xor rbx, rbx\n",
    ".Lshell_ls_record:\n",
    "    cmp rbx, r15\n",
    "    jge .Lshell_ls_done\n",
    // The kind of the entry.
    "    lea rsi, [r12 + 1024 + rbx]\n",
    "    mov edx, 1\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    // The name of the entry.
    "    movzx eax, byte ptr [r12 + 1024 + rbx + 1]\n",
    "    lea rsi, [r12 + 1024 + rbx + 2]\n",
    "    mov edx, eax\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    // The end of the line.
    "    lea rsi, [rip + .Ltext_newline]\n",
    "    mov edx, 1\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    // The next entry: the kind, the length of the name, and the name.
    "    movzx eax, byte ptr [r12 + 1024 + rbx + 1]\n",
    "    add rbx, 2\n",
    "    add rbx, rax\n",
    "    jmp .Lshell_ls_record\n",
    ".Lshell_ls_done:\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_ls_failed:\n",
    "    lea rsi, [rip + .Ltext_no_directory]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    // ---- cat ----
    ".Lshell_do_cat:\n",
    // The length of the arguments tells whether there is a file at all.
    "    cmp rbp, 0\n",
    "    je .Lshell_cat_usage\n",
    "    mov rdi, rbx\n",
    "    mov rsi, rbp\n",
    // open(path, path_length, 1): the file is opened for reading.
    "    mov edx, 1\n",
    "    mov eax, 3\n",
    "    int 0x80\n",
    "    cmp rax, 0\n",
    "    jl .Lshell_cat_failed\n",
    "    mov r14, rax\n",
    ".Lshell_cat_loop:\n",
    "    mov edi, r14d\n",
    "    lea rsi, [r12 + 3072]\n",
    "    mov edx, 512\n",
    "    mov eax, 4\n",
    "    int 0x80\n",
    "    cmp rax, 0\n",
    "    jle .Lshell_cat_done\n",
    "    mov rdx, rax\n",
    "    mov edi, 1\n",
    "    lea rsi, [r12 + 3072]\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    "    jmp .Lshell_cat_loop\n",
    ".Lshell_cat_done:\n",
    "    mov edi, r14d\n",
    "    mov eax, 6\n",
    "    int 0x80\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_cat_failed:\n",
    "    lea rsi, [rip + .Ltext_no_file]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    ".Lshell_cat_usage:\n",
    "    lea rsi, [rip + .Ltext_usage_cat]\n",
    "    call elvaris_shell_puts\n",
    "    jmp .Lshell_loop\n",
    // ---- the helpers ----
    // strcmp(rdi, rsi): the result is zero when the strings are equal.
    "elvaris_shell_strcmp:\n",
    ".Lstrcmp_loop:\n",
    "    mov al, byte ptr [rdi]\n",
    "    mov cl, byte ptr [rsi]\n",
    "    cmp al, cl\n",
    "    jne .Lstrcmp_diff\n",
    "    test al, al\n",
    "    jz .Lstrcmp_equal\n",
    "    inc rdi\n",
    "    inc rsi\n",
    "    jmp .Lstrcmp_loop\n",
    ".Lstrcmp_diff:\n",
    "    mov rax, 1\n",
    "    ret\n",
    ".Lstrcmp_equal:\n",
    "    xor rax, rax\n",
    "    ret\n",
    // puts(rsi): write a string that ends with a zero byte.
    "elvaris_shell_puts:\n",
    "    mov rdx, 0\n",
    ".Lputs_loop:\n",
    "    cmp byte ptr [rsi + rdx], 0\n",
    "    je .Lputs_write\n",
    "    inc rdx\n",
    "    jmp .Lputs_loop\n",
    ".Lputs_write:\n",
    "    mov edi, 1\n",
    "    mov eax, 1\n",
    "    int 0x80\n",
    "    ret\n",
    // ---- the texts ----
    // Every text ends with a zero byte, so that the helper that writes a
    // string knows where the string ends.
    ".Ltext_greeting:\n",
    "    .ascii \"Elvaris shell. Type help for the commands.\\n\"\n",
    "    .byte 0\n",
    ".Ltext_prompt:\n",
    "    .ascii \"sh> \"\n",
    ".Ltext_prompt_end:\n",
    "    .byte 0\n",
    ".Ltext_newline:\n",
    "    .ascii \"\\n\"\n",
    "    .byte 0\n",
    ".Ltext_help:\n",
    "    .ascii \"commands: help pwd ls cat echo ps mem clear shutdown reboot exit\\n\"\n",
    "    .byte 0\n",
    ".Ltext_root:\n",
    "    .ascii \"/\"\n",
    "    .byte 0\n",
    ".Ltext_status:\n",
    "    .ascii \"exit status: \"\n",
    "    .byte 0\n",
    ".Ltext_not_found:\n",
    "    .ascii \"sh: the command was not found\\n\"\n",
    "    .byte 0\n",
    ".Ltext_no_directory:\n",
    "    .ascii \"sh: the directory could not be read\\n\"\n",
    "    .byte 0\n",
    ".Ltext_no_file:\n",
    "    .ascii \"sh: the file could not be read\\n\"\n",
    "    .byte 0\n",
    ".Ltext_usage_cat:\n",
    "    .ascii \"sh: cat needs the name of a file\\n\"\n",
    "    .byte 0\n",
    ".Ltext_clear:\n",
    "    .ascii \"\\x1b[2J\\x1b[H\"\n",
    "    .byte 0\n",
    // The names of the commands.
    ".Lname_help:\n",
    "    .ascii \"help\"\n",
    "    .byte 0\n",
    ".Lname_pwd:\n",
    "    .ascii \"pwd\"\n",
    "    .byte 0\n",
    ".Lname_ls:\n",
    "    .ascii \"ls\"\n",
    "    .byte 0\n",
    ".Lname_cat:\n",
    "    .ascii \"cat\"\n",
    "    .byte 0\n",
    ".Lname_echo:\n",
    "    .ascii \"echo\"\n",
    "    .byte 0\n",
    ".Lname_ps:\n",
    "    .ascii \"ps\"\n",
    "    .byte 0\n",
    ".Lname_mem:\n",
    "    .ascii \"mem\"\n",
    "    .byte 0\n",
    ".Lname_clear:\n",
    "    .ascii \"clear\"\n",
    "    .byte 0\n",
    ".Lname_shutdown:\n",
    "    .ascii \"shutdown\"\n",
    "    .byte 0\n",
    ".Lname_reboot:\n",
    "    .ascii \"reboot\"\n",
    "    .byte 0\n",
    ".Lname_exit:\n",
    "    .ascii \"exit\"\n",
    "    .byte 0\n",
    ".globl elvaris_shell_text_end\n",
    "elvaris_shell_text_end:\n",
);

unsafe extern "C" {
    static elvaris_shell_program: u8;
    static elvaris_shell_text_end: u8;
}

/// The machine code of the program.
pub fn payload() -> &'static [u8] {
    // SAFETY: the two symbols are labels of the same assembly block, so the
    // range between them is the program. The program is never modified.
    unsafe {
        let start = core::ptr::addr_of!(elvaris_shell_program) as usize;
        let end = core::ptr::addr_of!(elvaris_shell_text_end) as usize;
        core::slice::from_raw_parts(start as *const u8, end - start)
    }
}
