# Linux syscall ABI (x86_64)

Living table. Numbers are the Linux x86_64 ABI. Status:

| Status | Meaning |
|---|---|
| **missing** | not wired; will `#UD` or never reached |
| **stub** | returns `-ENOSYS` (honest) |
| **done** | behaviour matches Linux enough for our tests |

Phase 0 has **no user syscalls**. The kernel is still the only code. This file exists so we do not invent a private ABI by accident.

| # | Name | Status | Test |
|---|---|---|---|
| 0 | `read` | missing | |
| 1 | `write` | missing | phase 2: hello ELF |
| 2 | `open` | missing | |
| 3 | `close` | missing | |
| 9 | `mmap` | missing | |
| 11 | `munmap` | missing | |
| 12 | `brk` | missing | |
| 17 | `pread64` | missing | |
| 24 | `sched_yield` | missing | |
| 28 | `madvise` | missing | |
| 39 | `getpid` | missing | |
| 56 | `clone` | missing | |
| 57 | `fork` | missing | phase 3 |
| 59 | `execve` | missing | phase 3 |
| 60 | `exit` | missing | phase 2 |
| 61 | `wait4` | missing | phase 3 |
| 63 | `uname` | missing | |
| 72 | `fcntl` | missing | |
| 76 | `truncate` | missing | |
| 79 | `getcwd` | missing | |
| 80 | `chdir` | missing | |
| 96 | `gettimeofday` | missing | |
| 158 | `arch_prctl` | missing | |
| 218 | `set_tid_address` | missing | |
| 228 | `clock_gettime` | missing | |
| 231 | `exit_group` | missing | |
| 257 | `openat` | missing | |
| 262 | `newfstatat` | missing | |
| 217 | `getdents64` | missing | |
| 16 | `ioctl` | missing | tty subset, phase 5 |
| 22 | `pipe` | missing | phase 3 |
| 32 | `dup` | missing | |
| 13 | `rt_sigaction` | missing | subset |
| 35 | `nanosleep` | missing | |
| 302 | `prlimit64` | missing | subset |

Anything not listed is missing. When a real ELF traps, add a row — implement or stub `ENOSYS`.
