//! Shell domain data: windows, tray icons, the app catalog, child
//! processes, and remembered notifications/toasts. Extracted from
//! `main.rs` verbatim (M1, behavior preserving).

use libwoven::{process, Spring};

use crate::geometry::Area;
use crate::tokens::{CATALOG, CLOSE_SIZE, MAX_APPS, MAX_APP_NAME, MAX_EXEC, MAX_TITLE, SHADOW_REACH, TITLE_HEIGHT};

#[derive(Clone, Copy)]
pub struct Window {
    pub owner: process::Id,
    /// Opt-in avoids sending unknown tags to strict legacy clients.
    pub cancellation_protocol: bool,
    pub token: u64,
    /// Top-left of the client area; the title bar sits above it.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub title: [u8; MAX_TITLE],
    pub title_length: usize,
    /// Opacity, 0 to 255: animated from 0 as the window opens.
    pub fade: Spring,
    /// The workspace the window belongs to.
    pub workspace: usize,
    /// Until this tick, a resize has been requested and the owner may be
    /// swapping surfaces: a failed blit of the old one does not mean it
    /// exited.
    pub resizing_until: libwoven::time::Ticks,
}

impl Window {
    pub fn alpha(&self) -> u8 {
        self.fade.value().clamp(0, 255) as u8
    }

    pub fn title_area(&self) -> Area {
        Area::new(self.x, self.y - TITLE_HEIGHT, self.width, TITLE_HEIGHT)
    }

    pub fn body(&self) -> Area {
        Area::new(self.x, self.y, self.width, self.height)
    }

    pub fn frame(&self) -> Area {
        self.title_area().union(self.body())
    }

    /// Everything the window and its deepest shadow can cover: what to
    /// repaint when it opens, closes, moves or changes elevation.
    pub fn footprint(&self) -> Area {
        let frame = self.frame();
        frame.union(Area::new(
            frame.x + SHADOW_REACH,
            frame.y + SHADOW_REACH,
            frame.width,
            frame.height,
        ))
    }

    /// The close control, or nothing on a window too narrow to carry one.
    pub fn close_area(&self) -> Area {
        if self.width < 4 * CLOSE_SIZE {
            return Area::new(0, 0, 0, 0);
        }
        Area::new(
            self.x + self.width - CLOSE_SIZE - 2,
            self.y - TITLE_HEIGHT + 2,
            CLOSE_SIZE,
            CLOSE_SIZE,
        )
    }
}

#[derive(Clone, Copy)]
pub struct TrayIcon {
    pub token: u64,
}

#[derive(Clone, Copy)]
pub struct App {
    name: [u8; MAX_APP_NAME],
    name_length: usize,
    exec: [u8; MAX_EXEC],
    exec_length: usize,
}

impl App {
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_length]
    }

    /// Parse a catalog entry: `Name=` and `Exec=` lines, both required.
    pub fn parse(text: &[u8]) -> Option<Self> {
        let mut app = Self {
            name: [0; MAX_APP_NAME],
            name_length: 0,
            exec: [0; MAX_EXEC],
            exec_length: 0,
        };
        for line in text.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if let Some(name) = line.strip_prefix(b"Name=") {
                if name.is_empty() || name.len() > MAX_APP_NAME {
                    return None;
                }
                app.name[..name.len()].copy_from_slice(name);
                app.name_length = name.len();
            } else if let Some(exec) = line.strip_prefix(b"Exec=") {
                if exec.is_empty() || exec.len() > MAX_EXEC {
                    return None;
                }
                app.exec[..exec.len()].copy_from_slice(exec);
                app.exec_length = exec.len();
            }
        }
        (app.name_length > 0 && app.exec_length > 0).then_some(app)
    }

    /// The command line, with `%p` replaced by the shell's process id.
    pub fn command<'a>(&self, shell: process::Id, buffer: &'a mut [u8; 128]) -> Option<&'a str> {
        let mut digits = [0u8; 20];
        let pid = libwoven::format_u64(shell.0, &mut digits);
        let exec = &self.exec[..self.exec_length];
        let mut length = 0;
        let mut index = 0;
        while index < exec.len() {
            let part: &[u8] = if exec[index..].starts_with(b"%p") {
                index += 2;
                pid
            } else {
                index += 1;
                &exec[index - 1..index]
            };
            buffer
                .get_mut(length..length + part.len())?
                .copy_from_slice(part);
            length += part.len();
        }
        core::str::from_utf8(&buffer[..length]).ok()
    }
}

/// Load the catalog, sorted by name. A missing catalog, or no `FileRead`,
/// leaves the launcher empty rather than stopping the shell.
/// The apps are written into `apps`, which starts empty; returns how many.
pub fn load_catalog(apps: &mut [Option<App>; MAX_APPS]) -> usize {
    let mut count = 0;
    let mut name = [0u8; libwoven::MAX_NAME];
    let mut index = 0;
    while count < MAX_APPS {
        let Ok((length, is_directory)) = libwoven::read_dir(CATALOG, index, &mut name) else {
            break;
        };
        index += 1;
        if is_directory || !name[..length].ends_with(b".app") {
            continue;
        }
        let mut path = [0u8; 128];
        let prefix = CATALOG.as_bytes();
        let total = prefix.len() + 1 + length;
        if total > path.len() {
            continue;
        }
        path[..prefix.len()].copy_from_slice(prefix);
        path[prefix.len()] = b'/';
        path[prefix.len() + 1..total].copy_from_slice(&name[..length]);
        let Ok(path) = core::str::from_utf8(&path[..total]) else {
            continue;
        };
        let Ok(descriptor) = libwoven::open(path) else {
            continue;
        };
        let mut text = [0u8; 256];
        let read = libwoven::read(descriptor, &mut text);
        let _ = libwoven::close(descriptor);
        let Some(app) = read.ok().and_then(|length| App::parse(&text[..length])) else {
            continue;
        };
        // Insertion sort by name, so the list reads alphabetically.
        let mut position = count;
        while position > 0 && apps[position - 1].is_some_and(|other: App| other.name() > app.name())
        {
            apps[position] = apps[position - 1];
            position -= 1;
        }
        apps[position] = Some(app);
        count += 1;
    }
    count
}

/// Whether `name` contains `query`, ignoring ASCII case.
pub fn matches(name: &[u8], query: &[u8]) -> bool {
    query.is_empty()
        || name
            .windows(query.len())
            .any(|window| window.eq_ignore_ascii_case(query))
}

#[derive(Clone, Copy)]
pub struct Child {
    pub pid: process::Id,
    pub app: usize,
}

/// One remembered notification, already formatted for its row:
/// `source: text`, cut to what fits.
#[derive(Clone, Copy)]
pub struct Note {
    pub text: [u8; 32],
    pub length: usize,
}

pub struct Toast {
    pub text: [u8; 32],
    pub length: usize,
    pub until: libwoven::time::Ticks,
}
