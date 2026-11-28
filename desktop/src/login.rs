use std::{io, path::PathBuf};

use crate::{
    abi,
    accounts::Users,
    storage, sys,
    ui::{self, Input, Key, Rect, Screen},
    x11,
};

const EXE: &str = "/bin/lumdesk";
const CARD: Rect = Rect::new(112, 66, 416, 268);
const SUBMIT: Rect = Rect::new(140, 278, 116, 26);
const ENTER: Rect = Rect::new(140, 162, 186, 34);
const CREATE: Rect = Rect::new(140, 212, 186, 34);
const PASSWORD: Rect = Rect::new(336, 212, 165, 34);

pub fn supervise() -> io::Result<()> {
    if sys::identity(0) != 0 {
        return Err(io::Error::other(
            "The login manager requires system authority",
        ));
    }
    let mut screen = Screen::open()?;
    status(
        &mut screen,
        "Starting Lumen",
        "Opening the NVMe account store",
    )?;
    let mut input = Input::new();
    let mut users = match storage::wait_for_volume().and_then(|()| Users::load()) {
        Ok(users) => users,
        Err(error) => {
            eprintln!("Lumdesk account store: {error}");
            status(&mut screen, "Account store unavailable", &error.to_string())?;
            loop {
                sys::sleep(100);
            }
        }
    };
    if users.is_empty() {
        println!("Lumdesk: first boot setup");
        create(&mut screen, &mut input, &mut users, true)?;
    }
    loop {
        println!("Lumdesk: sign in ready");
        let mut form = Form::new(false, &["Username", "Password"]);
        let account = loop {
            if !form.run(
                &mut screen,
                &mut input,
                "Sign in",
                "Lumen desktop",
                "Sign in",
            )? {
                continue;
            }
            status(&mut screen, "Signing in", "Verifying credentials")?;
            let result = users.authenticate(&form.fields[0].value, form.fields[1].value.as_bytes());
            form.fields[1].wipe();
            if let Some(user) = result {
                break user;
            }
            println!("Lumdesk: sign-in rejected");
            form.error = "Check your username and password".into();
            sys::sleep(500);
        };

        println!("Lumdesk: session menu uid={}", account.uid);
        if !session_menu(&mut screen, &mut input, &mut users, account)? {
            continue;
        }
        let name = username(account);
        let home = PathBuf::from("/home").join(&name);
        status(&mut screen, "Starting your session", "Restoring home files")?;
        if let Err(error) = storage::restore_home(account.uid, &home) {
            eprintln!("Lumdesk home restore: {error}");
            acknowledge(
                &mut screen,
                &mut input,
                "Home files unavailable",
                &error.to_string(),
            )?;
            continue;
        }
        screen.release();
        let session = if x11::installed() {
            "--x11-session"
        } else {
            "--session"
        };
        let result = sys::spawn(EXE, &[session], u64::from(account.uid), sys::SEAT)
            .and_then(|child| sys::wait(child, false));
        screen.acquire()?;
        match result {
            Ok(0) => {}
            Ok(status) => {
                acknowledge(
                    &mut screen,
                    &mut input,
                    "Session stopped",
                    &format!("Exit status {status}. Saving files."),
                )?;
            }
            Err(error) => {
                acknowledge(
                    &mut screen,
                    &mut input,
                    "Session failed to start",
                    &error.to_string(),
                )?;
                continue;
            }
        }
        // Retry on I/O errors while the RAMFS copy is still available.
        loop {
            status(&mut screen, "Signing out", "Saving home files to NVMe")?;
            match storage::backup_home(account.uid, &home) {
                Ok(()) => break,
                Err(error) => {
                    eprintln!("Lumdesk home backup: {error}");
                    acknowledge(
                        &mut screen,
                        &mut input,
                        "Home backup failed",
                        "Press Enter to retry",
                    )?;
                }
            }
        }
        println!("Lumdesk: signed out UID {}", account.uid);
    }
}

pub fn username(account: abi::Account) -> String {
    let count = account
        .name
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(account.name.len());
    String::from_utf8_lossy(&account.name[..count]).into_owned()
}

fn create(
    screen: &mut Screen,
    input: &mut Input,
    users: &mut Users,
    initial: bool,
) -> io::Result<()> {
    let mut form = Form::new(!initial, &["Username", "Password", "Repeat password"]);
    let title = if initial {
        "Create your administrator"
    } else {
        "Create a user"
    };
    let label = if initial {
        "First boot setup"
    } else {
        "Account management"
    };
    loop {
        if !form.run(screen, input, title, label, "Create")? {
            return Ok(());
        }
        if form.fields[1].value != form.fields[2].value {
            form.error = "The passwords differ".into();
        } else {
            status(
                screen,
                "Creating account",
                "Deriving password verifier and saving",
            )?;
            match users.create(
                &form.fields[0].value,
                form.fields[1].value.as_bytes(),
                initial,
            ) {
                Ok(()) => {
                    println!("Lumdesk: account saved");
                    return Ok(());
                }
                Err(error) => form.error = error.to_string(),
            }
        }
        form.fields[1].wipe();
        form.fields[2].wipe();
        form.focus = 1;
    }
}

fn session_menu(
    screen: &mut Screen,
    input: &mut Input,
    users: &mut Users,
    account: abi::Account,
) -> io::Result<bool> {
    loop {
        let admin = account.admin == 1;
        let title = if admin {
            "Administrator session"
        } else {
            "User session"
        };
        let subtitle = if admin {
            format!("{}  /  {} accounts", username(account), users.len())
        } else {
            username(account)
        };
        card(screen, title, &subtitle);
        screen.button(ENTER, "F1  Open desktop", true);
        if admin {
            screen.button(CREATE, "F2  Add account", false);
        }
        screen.button(PASSWORD, "F3  Change password", false);
        screen.text(140, 284, "Esc  Sign out", ui::MUTED, 40);
        screen.cursor(input.x, input.y);
        screen.present()?;
        let previous = input.buttons;
        while input.pointer() {}
        let clicked = input.buttons & 1 != 0 && previous & 1 == 0;
        match input.key() {
            Some(Key::Function(1) | Key::Enter) => return Ok(true),
            Some(Key::Function(2)) if admin => create(screen, input, users, false)?,
            Some(Key::Function(3)) => password(screen, input, users, account.uid)?,
            Some(Key::Escape) => return Ok(false),
            _ if clicked && ENTER.contains(input.x, input.y) => return Ok(true),
            _ if clicked && admin && CREATE.contains(input.x, input.y) => {
                create(screen, input, users, false)?
            }
            _ if clicked && PASSWORD.contains(input.x, input.y) => {
                password(screen, input, users, account.uid)?
            }
            _ => {}
        }
        sys::sleep(30);
    }
}

fn password(screen: &mut Screen, input: &mut Input, users: &mut Users, uid: u32) -> io::Result<()> {
    let mut form = Form::new(true, &["New password", "Repeat password"]);
    form.fields[0].secret = true;
    loop {
        if !form.run(
            screen,
            input,
            "Change your password",
            "Lumen account",
            "Save",
        )? {
            return Ok(());
        }
        if form.fields[0].value == form.fields[1].value {
            status(screen, "Changing password", "Saving the updated verifier")?;
            match users.change_password(uid, form.fields[0].value.as_bytes()) {
                Ok(()) => {
                    println!("Lumdesk: password updated uid={uid}");
                    return Ok(());
                }
                Err(error) => form.error = error.to_string(),
            }
        } else {
            form.error = "The passwords differ".into();
        }
        form.fields[0].wipe();
        form.fields[1].wipe();
        form.focus = 0;
    }
}

pub fn card(screen: &mut Screen, title: &str, subtitle: &str) {
    screen.background();
    screen.text(28, 26, "LUMEN", ui::ACCENT, 8);
    screen.rect(Rect::new(CARD.x + 4, CARD.y + 6, CARD.w, CARD.h), 0x0b121f);
    screen.rect(CARD, ui::FRAME);
    screen.rect(Rect::new(CARD.x, CARD.y, CARD.w, 2), ui::ACCENT);
    screen.text(140, 91, title, ui::TEXT, 44);
    screen.text(140, 114, subtitle, ui::MUTED, 44);
}

fn status(screen: &mut Screen, title: &str, subtitle: &str) -> io::Result<()> {
    card(screen, title, subtitle);
    screen.present()
}

fn acknowledge(
    screen: &mut Screen,
    input: &mut Input,
    title: &str,
    subtitle: &str,
) -> io::Result<()> {
    card(screen, title, subtitle);
    screen.button(SUBMIT, "Enter  OK", true);
    screen.present()?;
    loop {
        if matches!(input.key(), Some(Key::Enter | Key::Escape)) {
            return Ok(());
        }
        sys::sleep(20);
    }
}

struct Field {
    label: &'static str,
    secret: bool,
    value: String,
}

impl Field {
    fn wipe(&mut self) {
        // The field owns its byte buffer and a volatile store keeps the wipe
        // observable even when the String is about to be dropped.
        unsafe {
            for byte in self.value.as_mut_vec().iter_mut() {
                std::ptr::write_volatile(byte, 0);
            }
        }
        self.value.clear();
    }
}

impl Drop for Field {
    fn drop(&mut self) {
        if self.secret {
            self.wipe();
        }
    }
}

struct Form {
    fields: Vec<Field>,
    focus: usize,
    cancelable: bool,
    error: String,
}

impl Form {
    fn new(cancelable: bool, labels: &[&'static str]) -> Self {
        Self {
            cancelable,
            focus: 0,
            error: String::new(),
            fields: labels
                .iter()
                .enumerate()
                .map(|(i, label)| Field {
                    label,
                    secret: i != 0,
                    value: String::new(),
                })
                .collect(),
        }
    }

    fn field(index: usize) -> Rect {
        Rect::new(140, 143 + index as i32 * 37, 360, 29)
    }

    fn run(
        &mut self,
        screen: &mut Screen,
        input: &mut Input,
        title: &str,
        subtitle: &str,
        submit: &str,
    ) -> io::Result<bool> {
        let mut dirty = true;
        loop {
            if dirty {
                card(screen, title, subtitle);
                for (i, field) in self.fields.iter().enumerate() {
                    let rect = Self::field(i);
                    screen.rect(rect, ui::FIELD);
                    if self.focus == i {
                        screen.rect(
                            Rect::new(rect.x, rect.y + rect.h - 2, rect.w, 2),
                            ui::ACCENT,
                        );
                    }
                    screen.text(rect.x + 8, rect.y + 4, field.label, ui::MUTED, 24);
                    let content = if field.secret {
                        "*".repeat(field.value.len())
                    } else {
                        field.value.clone()
                    };
                    let content = &content[content.len().saturating_sub(43)..];
                    screen.text(rect.x + 8, rect.y + 16, content, ui::TEXT, 43);
                }
                screen.text(140, 258, &self.error, ui::ERROR, 44);
                screen.button(SUBMIT, submit, true);
                screen.text(
                    272,
                    286,
                    if self.cancelable {
                        "Tab next / Esc back"
                    } else {
                        "Tab next / Enter"
                    },
                    ui::MUTED,
                    25,
                );
                screen.cursor(input.x, input.y);
                screen.present()?;
                dirty = false;
            }

            let mut previous = input.buttons;
            while input.pointer() {
                dirty = true;
                if input.buttons & 1 != 0 && previous & 1 == 0 {
                    if SUBMIT.contains(input.x, input.y) {
                        return Ok(true);
                    }
                    if let Some(i) =
                        (0..self.fields.len()).find(|&i| Self::field(i).contains(input.x, input.y))
                    {
                        self.focus = i;
                    }
                }
                previous = input.buttons;
            }
            while let Some(key) = input.key() {
                dirty = true;
                let count = self.fields.len();
                let field = &mut self.fields[self.focus];
                match key {
                    Key::Tab | Key::Down => self.focus = (self.focus + 1) % count,
                    Key::Up => self.focus = (self.focus + count - 1) % count,
                    Key::Enter if self.focus + 1 == count => return Ok(true),
                    Key::Enter => self.focus += 1,
                    Key::Escape if self.cancelable => return Ok(false),
                    Key::Backspace => {
                        field.value.pop();
                    }
                    Key::Char(character)
                        if field.value.len() < if field.secret { 64 } else { 23 } =>
                    {
                        field.value.push(character);
                        self.error.clear();
                    }
                    _ => {}
                }
            }
            sys::sleep(28);
        }
    }
}
