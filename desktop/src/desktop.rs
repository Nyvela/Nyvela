use std::{
    collections::VecDeque,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    abi, sys,
    ui::{self, Input, Key, Rect, Screen},
};

const APPS: [&str; 5] = ["Files", "Notes", "Terminal", "Clock", "Doom"];
const TASKBAR: i32 = 366;
const TITLE: i32 = 24;
const EXE: &str = "/bin/lumdesk";

enum App {
    Files(Files),
    Notes(Notes),
    Terminal(Terminal),
    Clock { pid: u64, text: String },
}

struct Window {
    app: App,
    bounds: Rect,
    restored: Option<Rect>,
}

impl Window {
    fn name(&self) -> &str {
        match self.app {
            App::Files(_) => "Files",
            App::Notes(_) => "Notes",
            App::Terminal(_) => "Terminal",
            App::Clock { .. } => "Clock",
        }
    }

    fn close_rect(&self) -> Rect {
        Rect::new(
            self.bounds.x + self.bounds.w - 25,
            self.bounds.y + 3,
            21,
            18,
        )
    }

    fn maximize_rect(&self) -> Rect {
        Rect::new(
            self.bounds.x + self.bounds.w - 50,
            self.bounds.y + 3,
            21,
            18,
        )
    }

    fn grip(&self) -> Rect {
        Rect::new(
            self.bounds.x + self.bounds.w - 16,
            self.bounds.y + self.bounds.h - 16,
            16,
            16,
        )
    }

    fn maximize(&mut self) {
        self.bounds = if let Some(previous) = self.restored.take() {
            previous
        } else {
            let previous = self.bounds;
            self.restored = Some(previous);
            Rect::new(8, 27, ui::WIDTH - 16, TASKBAR - 34)
        };
    }

    fn draw(&self, screen: &mut Screen, active: bool) {
        let area = self.bounds;
        screen.rect(Rect::new(area.x + 4, area.y + 5, area.w, area.h), 0x080e19);
        screen.rect(area, ui::FRAME);
        screen.rect(
            Rect::new(area.x, area.y, area.w, TITLE),
            if active { 0x2d4870 } else { 0x202c43 },
        );
        if active {
            screen.rect(Rect::new(area.x, area.y, area.w, 2), ui::ACCENT);
        }
        screen.text(area.x + 10, area.y + 9, self.name(), ui::TEXT, 25);
        screen.button(
            self.maximize_rect(),
            if self.restored.is_some() { "-" } else { "+" },
            false,
        );
        screen.button(self.close_rect(), "x", false);
        let body = Rect::new(
            area.x + 7,
            area.y + TITLE + 5,
            area.w - 14,
            area.h - TITLE - 11,
        );
        match &self.app {
            App::Files(files) => files.draw(screen, body),
            App::Notes(notes) => notes.draw(screen, body),
            App::Terminal(terminal) => terminal.draw(screen, body),
            App::Clock { text, .. } => {
                screen.text(body.x + 8, body.y + 9, "SYSTEM UPTIME", ui::MUTED, 26);
                screen.text(body.x + 8, body.y + 33, text, ui::ACCENT, 28);
                screen.text(
                    body.x + 8,
                    body.y + 61,
                    "Clock client running over IPC",
                    ui::MUTED,
                    32,
                );
            }
        }
        let corner = self.grip();
        screen.rect(Rect::new(corner.x + 5, corner.y + 11, 7, 1), ui::MUTED);
        screen.rect(Rect::new(corner.x + 11, corner.y + 5, 1, 7), ui::MUTED);
    }
}

struct Desktop {
    screen: Screen,
    input: Input,
    windows: Vec<Window>,
    closed_clients: Vec<u64>,
    grab: Option<Grab>,
    home: PathBuf,
    user: String,
    banner: String,
}

#[derive(Clone, Copy)]
enum Grab {
    Move(i32, i32),
    Resize(i32, i32),
}

pub fn session() -> io::Result<()> {
    let mut desktop = Desktop {
        screen: Screen::open()?,
        input: Input::new(),
        windows: Vec::new(),
        closed_clients: Vec::new(),
        grab: None,
        home: PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into())),
        user: std::env::var("USER").unwrap_or_else(|_| "user".into()),
        banner: String::new(),
    };
    println!("Lumdesk: desktop ready uid={}", sys::identity(0));
    desktop.open(0);
    desktop.open(1);
    desktop.run()
}

impl Desktop {
    fn run(&mut self) -> io::Result<()> {
        let mut dirty = true;
        loop {
            while let Some(key) = self.input.key() {
                dirty = true;
                if self.key(key)? {
                    return Ok(());
                }
            }
            let mut previous = self.input.buttons;
            while self.input.pointer() {
                dirty = true;
                if self.input.buttons & 1 != 0 && previous & 1 == 0 {
                    if self.click()? {
                        return Ok(());
                    }
                }
                if self.input.buttons & 1 == 0 {
                    self.grab = None;
                } else {
                    self.drag();
                }
                previous = self.input.buttons;
            }
            dirty |= self.messages();
            if dirty {
                self.draw()?;
                dirty = false;
            }
            sys::sleep(16);
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        self.screen.background();
        self.screen.text(12, 10, "LUMEN DESKTOP", ui::ACCENT, 20);
        self.screen.text(
            390,
            10,
            &format!("{}  /  UID {}", self.user, sys::identity(0)),
            ui::MUTED,
            29,
        );
        for (i, window) in self.windows.iter().enumerate() {
            window.draw(&mut self.screen, i + 1 == self.windows.len());
        }
        if !self.banner.is_empty() {
            self.screen
                .rect(Rect::new(8, TASKBAR - 19, ui::WIDTH - 16, 16), ui::FIELD);
            self.screen
                .text(15, TASKBAR - 15, &self.banner, ui::ERROR, 74);
        }
        self.screen.rect(
            Rect::new(0, TASKBAR, ui::WIDTH, ui::HEIGHT - TASKBAR),
            0x111b2c,
        );
        self.screen
            .rect(Rect::new(0, TASKBAR, ui::WIDTH, 1), 0x405477);
        for (i, app) in APPS.iter().enumerate() {
            self.screen.button(button(i), app, false);
        }
        self.screen.button(button(5), "Sign out", true);
        self.screen.cursor(self.input.x, self.input.y);
        self.screen.present()
    }

    fn focus(&mut self, index: usize) {
        if index + 1 != self.windows.len() {
            let window = self.windows.remove(index);
            self.windows.push(window);
        }
    }

    fn open(&mut self, index: usize) {
        self.banner.clear();
        if let Some(index) = self
            .windows
            .iter()
            .position(|window| window.name() == APPS[index])
        {
            self.focus(index);
            return;
        }
        let (app, bounds) = match index {
            0 => (
                App::Files(Files::new(&self.home)),
                Rect::new(24, 42, 315, 245),
            ),
            1 => (
                App::Notes(Notes::new(&self.home)),
                Rect::new(220, 90, 346, 223),
            ),
            2 => (App::Terminal(Terminal::new()), Rect::new(97, 71, 437, 260)),
            3 => match sys::spawn(EXE, &["--clock-client"], u64::MAX, 0) {
                Ok(pid) => (
                    App::Clock {
                        pid,
                        text: "Starting clock...".into(),
                    },
                    Rect::new(335, 48, 276, 119),
                ),
                Err(error) => {
                    self.banner = error.to_string();
                    return;
                }
            },
            _ => return,
        };
        self.windows.push(Window {
            app,
            bounds,
            restored: None,
        });
    }

    fn close(&mut self) {
        if let Some(window) = self.windows.pop() {
            match window.app {
                App::Clock { pid, .. } => {
                    let _ = sys::send(pid as usize, b"close");
                    self.closed_clients.push(pid);
                }
                App::Terminal(terminal) => self.closed_clients.extend(terminal.jobs),
                App::Notes(mut notes) if notes.dirty => {
                    notes.save();
                    if notes.dirty {
                        self.banner = notes.status.clone();
                        self.windows.push(Window {
                            app: App::Notes(notes),
                            ..window
                        });
                    }
                }
                _ => {}
            }
        }
        self.grab = None;
    }

    fn key(&mut self, key: Key) -> io::Result<bool> {
        match key {
            Key::Function(1..=4) => {
                if let Key::Function(i) = key {
                    self.open(usize::from(i - 1));
                }
            }
            Key::Function(9) => self.doom()?,
            Key::Function(10) => return Ok(self.finish()),
            Key::Tab => {
                if self.windows.len() > 1 {
                    self.focus(0);
                }
            }
            Key::Control('w') => self.close(),
            Key::Function(11) => {
                if let Some(window) = self.windows.last_mut() {
                    window.maximize();
                }
            }
            _ => {
                if let Some(window) = self.windows.last_mut() {
                    match &mut window.app {
                        App::Files(files) => files.key(key),
                        App::Notes(notes) => notes.key(key),
                        App::Terminal(terminal) => terminal.key(key),
                        App::Clock { .. } => {}
                    }
                }
            }
        }
        Ok(false)
    }

    fn click(&mut self) -> io::Result<bool> {
        let (x, y) = (self.input.x, self.input.y);
        if y >= TASKBAR {
            if let Some(index) = (0..=5).find(|&i| button(i).contains(x, y)) {
                match index {
                    4 => self.doom()?,
                    5 => return Ok(self.finish()),
                    _ => self.open(index),
                }
            }
            return Ok(false);
        }
        let Some(index) = self
            .windows
            .iter()
            .rposition(|window| window.bounds.contains(x, y))
        else {
            return Ok(false);
        };
        self.focus(index);
        let window = self.windows.last_mut().unwrap();
        if window.close_rect().contains(x, y) {
            self.close();
        } else if window.maximize_rect().contains(x, y) {
            window.maximize();
        } else if window.restored.is_none() && window.grip().contains(x, y) {
            self.grab = Some(Grab::Resize(window.bounds.w - x, window.bounds.h - y));
        } else if window.restored.is_none() && y < window.bounds.y + TITLE {
            self.grab = Some(Grab::Move(x - window.bounds.x, y - window.bounds.y));
        } else if let App::Files(files) = &mut window.app {
            let start = window.bounds.y + TITLE + 37;
            if y >= start {
                let row = ((y - start) / 14) as usize;
                files.click(
                    row + files.first_visible((window.bounds.h - TITLE - 50).max(14) as usize / 14),
                );
            }
        }
        Ok(false)
    }

    fn drag(&mut self) {
        let Some(window) = self.windows.last_mut() else {
            return;
        };
        let (x, y) = (self.input.x, self.input.y);
        match self.grab {
            Some(Grab::Move(dx, dy)) => {
                window.bounds.x = (x - dx).clamp(0, ui::WIDTH - window.bounds.w);
                window.bounds.y = (y - dy).clamp(26, TASKBAR - window.bounds.h);
            }
            Some(Grab::Resize(dw, dh)) => {
                window.bounds.w = (x + dw).clamp(
                    240.min(ui::WIDTH - window.bounds.x),
                    ui::WIDTH - window.bounds.x,
                );
                window.bounds.h = (y + dh).clamp(
                    105.min(TASKBAR - window.bounds.y),
                    TASKBAR - window.bounds.y,
                );
            }
            None => {}
        }
    }

    fn messages(&mut self) -> bool {
        let mut changed = false;
        let mut buffer = [0; abi::IPC_MAX];
        for _ in 0..32 {
            let Some((sender, length)) = sys::message(&mut buffer) else {
                break;
            };
            for window in &mut self.windows {
                match &mut window.app {
                    App::Clock { pid, text } if *pid == sender as u64 => {
                        *text = String::from_utf8_lossy(&buffer[..length])
                            .chars()
                            .take(28)
                            .collect();
                        changed = true;
                    }
                    App::Terminal(terminal) if terminal.jobs.contains(&(sender as u64)) => {
                        terminal.append(&String::from_utf8_lossy(&buffer[..length]));
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        self.closed_clients.retain(
            |&pid| matches!(sys::wait(pid, true), Err(error) if error.raw_os_error() == Some(11)),
        );
        for window in &mut self.windows {
            if let App::Terminal(terminal) = &mut window.app {
                changed |= terminal.reap();
            }
        }
        changed
    }

    fn finish(&mut self) -> bool {
        for window in &mut self.windows {
            if let App::Notes(notes) = &mut window.app
                && notes.dirty
            {
                notes.save();
                if notes.dirty {
                    self.banner = notes.status.clone();
                    return false;
                }
            }
        }
        true
    }

    fn doom(&mut self) -> io::Result<()> {
        if !Path::new("/freedoom1.wad").exists() {
            self.banner = "The Freedoom WAD is missing".into();
            return Ok(());
        }
        self.screen.release();
        let result = sys::spawn(
            "/bin/doom",
            &["-iwad", "/freedoom1.wad", "-nosound"],
            u64::MAX,
            sys::SEAT,
        )
        .and_then(|pid| sys::wait(pid, false));
        self.screen.acquire()?;
        self.input = Input::new();
        self.grab = None;
        self.banner = match result {
            Ok(0) => String::new(),
            Ok(code) => format!("Doom exited with status {code}"),
            Err(error) => error.to_string(),
        };
        Ok(())
    }
}

fn button(i: usize) -> Rect {
    if i == 5 {
        Rect::new(536, TASKBAR + 5, 95, 24)
    } else {
        Rect::new(8 + i as i32 * 91, TASKBAR + 5, 84, 24)
    }
}

struct Files {
    root: PathBuf,
    path: PathBuf,
    items: Vec<(String, bool)>,
    selected: usize,
    preview: Option<String>,
    status: String,
}

impl Files {
    fn new(home: &Path) -> Self {
        let mut files = Self {
            root: home.into(),
            path: home.into(),
            items: Vec::new(),
            selected: 0,
            preview: None,
            status: String::new(),
        };
        files.reload();
        files
    }

    fn reload(&mut self) {
        self.items.clear();
        self.preview = None;
        self.selected = 0;
        self.status.clear();
        match fs::read_dir(&self.path) {
            Ok(directory) => {
                for file in directory.take(256) {
                    match file {
                        Ok(file) => self.items.push((
                            file.file_name().to_string_lossy().into_owned(),
                            file.file_type().is_ok_and(|kind| kind.is_dir()),
                        )),
                        Err(error) => {
                            self.status = error.to_string();
                            break;
                        }
                    }
                }
                self.items
                    .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn enter(&mut self) {
        if let Some((name, directory)) = self.items.get(self.selected) {
            let path = self.path.join(name);
            if *directory {
                self.path = path;
                self.reload();
            } else {
                self.status.clear();
                self.preview = match fs::File::open(&path) {
                    Ok(file) => {
                        use std::io::Read;
                        let mut content = Vec::new();
                        match file.take(2048).read_to_end(&mut content) {
                            Ok(_) => Some(String::from_utf8_lossy(&content).into_owned()),
                            Err(error) => {
                                self.status = error.to_string();
                                None
                            }
                        }
                    }
                    Err(error) => {
                        self.status = error.to_string();
                        None
                    }
                };
            }
        }
    }

    fn first_visible(&self, rows: usize) -> usize {
        self.selected.saturating_add(1).saturating_sub(rows)
    }

    fn click(&mut self, row: usize) {
        if self.preview.is_none() && row < self.items.len() {
            if row == self.selected {
                self.enter();
            } else {
                self.selected = row;
            }
        }
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down if self.selected + 1 < self.items.len() => self.selected += 1,
            Key::Enter | Key::Right => self.enter(),
            Key::Escape if self.preview.is_some() => self.preview = None,
            Key::Left | Key::Backspace => {
                if self.preview.take().is_none() && self.path != self.root {
                    self.path.pop();
                    self.reload();
                }
            }
            Key::Function(5) => self.reload(),
            _ => {}
        }
    }

    fn draw(&self, screen: &mut Screen, area: Rect) {
        let cols = ((area.w - 12) / 8) as usize;
        screen.text(
            area.x + 5,
            area.y + 4,
            &self.path.display().to_string(),
            ui::ACCENT,
            cols,
        );
        screen.text(
            area.x + 5,
            area.y + 18,
            "Enter open / Backspace up / F5 refresh",
            ui::MUTED,
            cols,
        );
        if !self.status.is_empty() {
            screen.text(area.x + 5, area.y + 35, &self.status, ui::ERROR, cols);
            return;
        }
        let count = ((area.h - 35).max(1) / 14) as usize;
        if let Some(preview) = &self.preview {
            for (row, line) in preview.lines().take(count).enumerate() {
                screen.text(
                    area.x + 5,
                    area.y + 34 + row as i32 * 14,
                    line,
                    ui::TEXT,
                    cols,
                );
            }
        } else if self.items.is_empty() {
            screen.text(
                area.x + 5,
                area.y + 35,
                "This folder is empty",
                ui::MUTED,
                cols,
            );
        } else {
            let start = self.first_visible(count);
            for (row, (name, directory)) in self.items.iter().skip(start).take(count).enumerate() {
                let y = area.y + 32 + row as i32 * 14;
                if row + start == self.selected {
                    screen.rect(Rect::new(area.x, y, area.w, 13), ui::FIELD);
                }
                screen.text(
                    area.x + 5,
                    y + 2,
                    &format!("{name}{}", if *directory { "/" } else { "" }),
                    ui::TEXT,
                    cols,
                );
            }
        }
    }
}

struct Notes {
    path: PathBuf,
    text: String,
    dirty: bool,
    status: String,
}

impl Notes {
    fn new(home: &Path) -> Self {
        let path = home.join("notes.txt");
        let (text, status) = match fs::read_to_string(&path) {
            Ok(text) => (text, String::from("Ctrl+S save")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (String::new(), String::from("Ctrl+S save"))
            }
            Err(error) => (String::new(), error.to_string()),
        };
        Self {
            path,
            text,
            dirty: false,
            status,
        }
    }

    fn save(&mut self) {
        match fs::write(&self.path, &self.text) {
            Ok(()) => {
                self.dirty = false;
                self.status = "Saved to notes.txt".into();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Control('s') => self.save(),
            Key::Char(character) if self.text.len() < 32 * 1024 => {
                self.text.push(character);
                self.dirty = true;
            }
            Key::Enter if self.text.len() < 32 * 1024 => {
                self.text.push('\n');
                self.dirty = true;
            }
            Key::Backspace => self.dirty |= self.text.pop().is_some(),
            _ => {}
        }
    }

    fn draw(&self, screen: &mut Screen, area: Rect) {
        let cols = ((area.w - 12) / 8) as usize;
        screen.rect(area, 0x131e30);
        let rows = ((area.h - 24).max(0) / 12) as usize;
        let visible: Vec<Vec<char>> = self
            .text
            .split('\n')
            .flat_map(|line| {
                let chars: Vec<char> = line.chars().collect();
                if chars.is_empty() {
                    vec![Vec::new()]
                } else {
                    chars
                        .chunks(cols.max(1))
                        .map(|part| part.to_vec())
                        .collect()
                }
            })
            .collect();
        for (index, line) in visible
            .iter()
            .skip(visible.len().saturating_sub(rows))
            .enumerate()
        {
            let line: String = line.iter().collect();
            screen.text(
                area.x + 6,
                area.y + 6 + index as i32 * 12,
                &line,
                ui::TEXT,
                cols,
            );
        }
        let status = if self.dirty {
            "Unsaved changes  /  Ctrl+S save"
        } else {
            &self.status
        };
        screen.text(area.x + 6, area.y + area.h - 12, status, ui::MUTED, cols);
    }
}

struct Terminal {
    lines: VecDeque<String>,
    command: String,
    jobs: Vec<u64>,
}

impl Terminal {
    fn new() -> Self {
        Self {
            lines: ["Type help for commands".into(), String::new()].into(),
            command: String::new(),
            jobs: Vec::new(),
        }
    }

    fn append(&mut self, text: &str) {
        for character in text.chars() {
            match character {
                '\n' => self.lines.push_back(String::new()),
                '\r' => {}
                character if character.is_ascii_graphic() || character == ' ' => {
                    let line = self.lines.back_mut().unwrap();
                    if line.len() >= 100 {
                        self.lines.push_back(String::new());
                    }
                    self.lines.back_mut().unwrap().push(character);
                }
                _ => {}
            }
            if self.lines.len() > 200 {
                self.lines.pop_front();
            }
        }
    }

    fn execute(&mut self) {
        let line = std::mem::take(&mut self.command);
        self.append(&format!("> {line}\n"));
        let Some(args) = words(&line) else {
            self.append("Unclosed quote\n");
            return;
        };
        let Some((command, rest)) = args.split_first() else {
            return;
        };
        match command.as_str() {
            "help" => self.append(
                "ls [folder], cat <file>, mkdir <folder>, whoami\nrun <program> [args], clear\n",
            ),
            "clear" => self.lines = [String::new()].into(),
            "whoami" => self.append(&format!(
                "{} (uid {})\n",
                std::env::var("USER").unwrap_or_default(),
                sys::identity(0)
            )),
            "ls" => {
                let path = rest.first().map_or(".", String::as_str);
                match fs::read_dir(path) {
                    Ok(dir) => {
                        for item in dir.take(96) {
                            match item {
                                Ok(item) => self
                                    .append(&format!("{}\n", item.file_name().to_string_lossy())),
                                Err(error) => self.append(&format!("{error}\n")),
                            }
                        }
                    }
                    Err(error) => self.append(&format!("{error}\n")),
                }
            }
            "cat" if rest.len() == 1 => {
                use std::io::Read;
                let result = fs::File::open(&rest[0]).and_then(|file| {
                    let mut output = Vec::new();
                    file.take(8192).read_to_end(&mut output)?;
                    Ok(output)
                });
                match result {
                    Ok(data) => {
                        self.append(&String::from_utf8_lossy(&data));
                        self.append("\n");
                    }
                    Err(error) => self.append(&format!("{error}\n")),
                }
            }
            "mkdir" if rest.len() == 1 => {
                if let Err(error) = fs::create_dir(&rest[0]) {
                    self.append(&format!("{error}\n"));
                }
            }
            "run" if !rest.is_empty() => {
                let path = if rest[0].starts_with('/') {
                    rest[0].clone()
                } else {
                    format!("/bin/{}", rest[0])
                };
                let arguments: Vec<&str> = rest[1..].iter().map(String::as_str).collect();
                match sys::spawn(&path, &arguments, u64::MAX, sys::CAPTURE) {
                    Ok(pid) => self.jobs.push(pid),
                    Err(error) => self.append(&format!("{error}\n")),
                }
            }
            _ => self.append("Unknown or incomplete command. Type help.\n"),
        }
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Enter => self.execute(),
            Key::Backspace => {
                self.command.pop();
            }
            Key::Char(character) if self.command.len() < 256 => self.command.push(character),
            _ => {}
        }
    }

    fn reap(&mut self) -> bool {
        let mut completed = Vec::new();
        self.jobs.retain(|&pid| match sys::wait(pid, true) {
            Ok(status) => {
                completed.push((pid, status));
                false
            }
            Err(error) => error.raw_os_error() == Some(11),
        });
        let changed = !completed.is_empty();
        for (pid, status) in completed {
            self.append(&format!("[{pid}] exit {status}\n"));
        }
        changed
    }

    fn draw(&self, screen: &mut Screen, area: Rect) {
        screen.rect(area, 0x0b1322);
        let cols = ((area.w - 12) / 8).max(1) as usize;
        let rows = ((area.h - 25).max(0) / 12) as usize;
        let rendered: Vec<&str> = self
            .lines
            .iter()
            .flat_map(|line| {
                if line.is_empty() {
                    vec![""]
                } else {
                    line.as_bytes()
                        .chunks(cols)
                        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("?"))
                        .collect()
                }
            })
            .collect();
        for (row, line) in rendered
            .iter()
            .skip(rendered.len().saturating_sub(rows))
            .enumerate()
        {
            screen.text(
                area.x + 6,
                area.y + 5 + 12 * row as i32,
                line,
                ui::TEXT,
                cols,
            );
        }
        let visible = &self.command[self.command.len().saturating_sub(cols.saturating_sub(2))..];
        screen.text(
            area.x + 6,
            area.y + area.h - 13,
            &format!("> {visible}"),
            ui::ACCENT,
            cols,
        );
    }
}

fn words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut present = false;
    for character in line.chars() {
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            } else {
                word.push(character);
            }
        } else if matches!(character, '"' | '\'') {
            quote = Some(character);
            present = true;
        } else if character.is_whitespace() {
            if present {
                words.push(std::mem::take(&mut word));
                present = false;
            }
        } else {
            word.push(character);
            present = true;
        }
    }
    if quote.is_some() {
        return None;
    }
    if present {
        words.push(word);
    }
    Some(words)
}

pub fn clock_client() -> io::Result<()> {
    let parent = sys::identity(2) as usize;
    let mut incoming = [0; abi::IPC_MAX];
    let mut previous = u64::MAX;
    loop {
        if let Some((sender, count)) = sys::message(&mut incoming)
            && sender == parent
            && &incoming[..count] == b"close"
        {
            return Ok(());
        }
        let now = sys::uptime() / 1000;
        if now != previous {
            let time = format!("{:02}:{:02}:{:02}", now / 3600, now % 3600 / 60, now % 60);
            sys::send(parent, time.as_bytes())
                .map_err(|_| io::Error::other("window server unavailable"))?;
            previous = now;
        }
        sys::sleep(100);
    }
}
