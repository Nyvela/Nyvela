//! Nyvela shell (Rust): minimal ring3 command interpreter.
//!
//! Commands: help, ls [path], cat <file>, echo <...>, run <name>, clear, exit.
//! Port of the original C shell; behavior is identical.

#![no_std]
#![no_main]

mod sys;

use sys::*;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Keep infallible: fixed bytes + exit, no formatting.
    sys_write(1, b"panic\n");
    sys_exit(1);
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    sh_main();
}

fn write_str(s: &[u8]) {
    sys_write(1, s);
}

fn write_code(prefix: &[u8], v: i64) {
    let mut num = [0u8; 24];
    let n = itoa(v, &mut num);
    write_str(prefix);
    sys_write(1, &num[..n]);
    write_str(b"\n");
}

fn errstr(e: i64) -> &'static [u8] {
    match e {
        -1 => b"invalid argument",
        -2 => b"no such file or directory",
        -3 => b"already exists",
        -4 => b"not a directory",
        -5 => b"is a directory",
        -6 => b"too big",
        -7 => b"bad path",
        _ => b"unknown error",
    }
}

fn puterr(cmd: &[u8], path: &[u8], e: i64) {
    write_str(cmd);
    write_str(b": ");
    write_str(path);
    write_str(b": ");
    write_str(errstr(e));
    write_str(b"\n");
}

/// Resolve a NUL-terminated input path into `out` (NUL-terminated):
/// absolute stays, relative gains '/'; trailing slashes stripped
/// (except root). Returns length excl. NUL, or `None` when too long.
fn resolve_path(inp: &[u8], out: &mut [u8; 256]) -> Option<usize> {
    let mut n = 0usize;
    let mut i = 0usize;

    if inp.first().copied().unwrap_or(0) != b'/' {
        if out.len() < 2 {
            return None;
        }
        out[0] = b'/';
        n = 1;
    }

    while i < inp.len() && inp[i] != 0 {
        if n + 1 >= out.len() {
            return None;
        }
        out[n] = inp[i];
        n += 1;
        i += 1;
    }

    while n > 1 && out[n - 1] == b'/' {
        n -= 1;
    }

    if n < out.len() {
        out[n] = 0;
    } else {
        return None;
    }

    Some(n)
}

fn cmd_help() {
    write_str(b"commands:\n");
    write_str(b"  help          this text\n");
    write_str(b"  ls [path]     list directory (default /, relative ok)\n");
    write_str(b"  cat <file>    print file (relative ok)\n");
    write_str(b"  echo <...>    print args\n");
    write_str(b"  run <name>    run /bin/<name>, print exit code\n");
    write_str(b"  clear         clear screen\n");
    write_str(b"  exit          exit shell\n");
}

fn cmd_ls(arg: Option<&[u8]>, list: &mut [u8; 1024], path: &mut [u8; 256]) {
    let raw: &[u8] = arg.unwrap_or(b"/\0");
    if resolve_path(raw, path).is_none() {
        write_str(b"ls: path too long\n");
        return;
    }
    let r = sys_fs_list(&path[..], &mut list[..]);
    if r < 0 {
        puterr(b"ls", path_nul(path), r);
        return;
    }
    if r == 0 {
        write_str(b"(empty)\n");
        return;
    }
    sys_write(1, &list[..r as usize]);
}

/// `path` is a NUL-terminated buffer; returns it as a slice excl. NUL.
fn path_nul(path: &[u8]) -> &[u8] {
    &path[..slen(path)]
}

fn cmd_cat(arg: Option<&[u8]>, io: &mut [u8; 512], path: &mut [u8; 256]) {
    let raw = match arg {
        Some(a) => a,
        None => {
            write_str(b"usage: cat <file>\n");
            return;
        }
    };
    if resolve_path(raw, path).is_none() {
        write_str(b"cat: path too long\n");
        return;
    }
    let mut off: u64 = 0;
    loop {
        let r = sys_fs_read(&path[..], &mut io[..], off);
        if r < 0 {
            puterr(b"cat", path_nul(path), r);
            return;
        }
        if r == 0 {
            break;
        }
        sys_write(1, &io[..r as usize]);
        off += r as u64;
        if r < io.len() as i64 {
            break;
        }
    }
}

fn cmd_echo(args: &[Arg], line: &[u8]) {
    let mut first = true;
    for a in args {
        if !first {
            write_str(b" ");
        }
        first = false;
        write_str(&line[a.0..a.0 + a.1]);
    }
    write_str(b"\n");
}

fn cmd_run(arg: Option<Arg>, line: &[u8], prog: &mut [u8; 256]) {
    let span = match arg {
        Some(a) => a,
        None => {
            write_str(b"usage: run <name>  (runs /bin/<name>)\n");
            return;
        }
    };
    let a = &line[span.0..span.0 + span.1];

    if a.first().copied() == Some(b'/') {
        // Absolute: only /bin/ (executing a data file faults on garbage).
        if a.len() < 5 || &a[..5] != b"/bin/" {
            write_str(b"run: only /bin/ programs (try bare name)\n");
            return;
        }
        // Copy + NUL-terminate into prog.
        if a.len() + 1 > prog.len() {
            write_str(b"run: path too long\n");
            return;
        }
        let mut n = 0;
        while n < a.len() {
            prog[n] = a[n];
            n += 1;
        }
        prog[n] = 0;
    } else {
        for &c in a {
            if c == b'/' {
                write_str(b"run: use bare name or /bin/ path\n");
                return;
            }
        }
        let pre = b"/bin/";
        if pre.len() + a.len() + 1 > prog.len() {
            write_str(b"run: name too long\n");
            return;
        }
        let mut n = 0;
        for &c in pre {
            prog[n] = c;
            n += 1;
        }
        for &c in a {
            prog[n] = c;
            n += 1;
        }
        prog[n] = 0;
    }

    let code = sys_exec(&prog[..]);
    if code < 0 {
        puterr(b"run", path_nul(prog), code);
        return;
    }
    write_code(b"exit: ", code);
}

fn cmd_clear() {
    for _ in 0..25 {
        write_str(b"\n");
    }
}

type Arg = (usize, usize); // (start, len) span into line buffer

/// Reads a line with echo + backspace into `line`. Returns length.
fn read_line(line: &mut [u8; 256]) -> usize {
    let mut len = 0usize;
    loop {
        let mut c = [0u8; 1];
        let r = sys_read(0, &mut c);
        if r < 0 {
            write_code(b"\ninput error ", r);
            return 0;
        }
        if r == 0 {
            continue;
        }
        let ch = c[0];
        if ch == b'\n' {
            write_str(b"\n");
            break;
        }
        if ch == 0x08 {
            if len > 0 {
                len -= 1;
                sys_write(1, &[0x08]);
            }
            continue;
        }
        if !(32..=126).contains(&ch) {
            continue;
        }
        if len + 1 >= line.len() {
            continue;
        }
        line[len] = ch;
        len += 1;
        sys_write(1, &[ch]);
    }
    len
}

/// Split line[..len] on spaces/tabs into spans. Returns argc.
fn split(line: &[u8], len: usize, argv: &mut [Arg; 8]) -> usize {
    let mut argc = 0usize;
    let mut i = 0usize;
    while i < len {
        while i < len && (line[i] == b' ' || line[i] == b'\t') {
            i += 1;
        }
        if i >= len {
            break;
        }
        if argc >= argv.len() {
            break;
        }
        let s = i;
        while i < len && line[i] != b' ' && line[i] != b'\t' {
            i += 1;
        }
        argv[argc] = (s, i - s);
        argc += 1;
    }
    argc
}

fn eq_span(line: &[u8], a: &Arg, lit: &[u8]) -> bool {
    if a.1 != lit.len() {
        return false;
    }
    &line[a.0..a.0 + a.1] == lit
}

/// NUL-terminate argv span `a` in place (line buffer is ours).
fn nul_term(line: &mut [u8], a: &Arg) {
    let e = a.0 + a.1;
    if e < line.len() {
        line[e] = 0;
    }
}

fn run_line(
    line: &mut [u8; 256],
    len: usize,
    list: &mut [u8; 1024],
    io: &mut [u8; 512],
    prog: &mut [u8; 256],
    path: &mut [u8; 256],
) {
    let mut argv: [Arg; 8] = [(0, 0); 8];
    let argc = split(&line[..], len, &mut argv);
    if argc == 0 {
        return;
    }
    // NUL-terminate each span so syscalls get C strings.
    for k in 0..argc {
        nul_term(line, &argv[k]);
    }

    if eq_span(line, &argv[0], b"help") {
        cmd_help();
    } else if eq_span(line, &argv[0], b"ls") {
        let a = if argc > 1 {
            Some(&line[argv[1].0..argv[1].0 + argv[1].1 + 1])
        } else {
            None
        };
        cmd_ls(a, list, path);
    } else if eq_span(line, &argv[0], b"cat") {
        let a = if argc > 1 {
            Some(&line[argv[1].0..argv[1].0 + argv[1].1 + 1])
        } else {
            None
        };
        cmd_cat(a, io, path);
    } else if eq_span(line, &argv[0], b"echo") {
        cmd_echo(&argv[1..argc], &line[..]);
    } else if eq_span(line, &argv[0], b"run") {
        let a = if argc > 1 { Some(argv[1]) } else { None };
        cmd_run(a, &line[..], prog);
    } else if eq_span(line, &argv[0], b"clear") {
        cmd_clear();
    } else if eq_span(line, &argv[0], b"exit") {
        sys_exit(0);
    } else {
        write_str(b"unknown command: ");
        write_str(&line[argv[0].0..argv[0].0 + argv[0].1]);
        write_str(b"\n");
    }
}

fn sh_main() -> ! {
    let mut line = [0u8; 256];
    let mut list = [0u8; 1024];
    let mut io = [0u8; 512];
    let mut prog = [0u8; 256];
    let mut path = [0u8; 256];

    write_str(b"Nyvela shell. Type 'help'.\n");
    loop {
        write_str(b"nyvela> ");
        let len = read_line(&mut line);
        run_line(&mut line, len, &mut list, &mut io, &mut prog, &mut path);
    }
}
