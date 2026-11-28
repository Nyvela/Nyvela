#[path = "../../sdk/abi.rs"]
mod abi;
mod disk {
    include!(env!("LUMEN_DISK_SOURCE"));
}
#[path = "../../sdk/sys.rs"]
mod trap;

mod accounts;
mod desktop;
mod login;
mod storage;
mod sys;
mod ui;
mod x11;

fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("--session") => desktop::session(),
        Some("--x11-session") => x11::session(),
        Some("--clock-client") => desktop::clock_client(),
        Some(_) => Err(std::io::Error::other("unsupported Lumdesk mode")),
        None => login::supervise(),
    };
    if let Err(error) = result {
        eprintln!("Lumdesk: {error}");
        std::process::exit(1);
    }
}
