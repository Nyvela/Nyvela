use std::io::{self, Write};

fn main() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "Hello from Rust std on Lumen!")?;
    if !arguments.is_empty() {
        writeln!(stdout, "Arguments: {arguments:?}")?;
    }
    Ok(())
}
