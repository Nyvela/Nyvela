use std::{io, path::Path};

use crate::{abi, sys};

const SERVER: &str = "/bin/Xvfb";
const MANAGER: &str = "/bin/jwm";
const CONFIG: &str = "/usr/share/X11/jwmrc";
const SOCKET: &[u8] = b"/tmp/.X11-unix/X0";

pub fn installed() -> bool {
    Path::new(SERVER).is_file() && Path::new(MANAGER).is_file()
}

pub fn session() -> io::Result<()> {
    let geometry = format!("{}x{}x24", abi::DISPLAY_WIDTH, abi::DISPLAY_HEIGHT);
    let server = sys::spawn(
        SERVER,
        &[
            ":0",
            "-screen",
            "0",
            &geometry,
            "-nolisten",
            "tcp",
            "-noreset",
        ],
        u64::MAX,
        sys::SEAT,
    )?;
    wait_for_server(server)?;
    let manager = sys::spawn(MANAGER, &["-display", ":0", "-f", CONFIG], u64::MAX, 0)?;
    println!("X11 session: started JWM for UID {}", sys::identity(0));

    loop {
        if let Some(status) = exited(manager)? {
            return if status == 0 {
                Ok(())
            } else {
                Err(io::Error::other(format!("JWM exited with status {status}")))
            };
        }
        if let Some(status) = exited(server)? {
            return Err(io::Error::other(format!(
                "X.Org exited with status {status}"
            )));
        }
        sys::sleep(100);
    }
}

fn exited(pid: u64) -> io::Result<Option<u64>> {
    match sys::wait(pid, true) {
        Ok(status) => Ok(Some(status)),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    }
}

fn wait_for_server(server: u64) -> io::Result<()> {
    let mut endpoint = abi::LocalEndpoint::EMPTY;
    endpoint.name[..SOCKET.len()].copy_from_slice(SOCKET);
    let deadline = sys::uptime() + 10_000;
    loop {
        if let Some(status) = exited(server)? {
            return Err(io::Error::other(format!(
                "X.Org exited with status {status}"
            )));
        }
        let fd = sys::checked(sys::call(
            abi::OPEN_SOCKET,
            [abi::SOCKET_LOCAL_STREAM, 0, 0, 0, 0, 0],
        ))?;
        let result = sys::call(
            abi::CONNECT_SOCKET,
            [fd, endpoint.name.as_ptr().addr() as u64, 0, 0, 0, 0],
        );
        sys::call(abi::CLOSE, [fd, 0, 0, 0, 0, 0]);
        if result == 0 {
            return Ok(());
        }
        if result != -111 && result != -11 {
            return sys::checked(result).map(|_| ());
        }
        if sys::uptime() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "waiting for the X11 socket",
            ));
        }
        sys::sleep(50);
    }
}
