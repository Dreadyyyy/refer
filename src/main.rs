pub mod cursor;
pub mod input;
pub mod resource;
mod ui;

use std::io::{stdout, Stdout};
use std::ops::Drop;
use std::sync::{Arc, Mutex};

use clap::Parser;
use crossterm::{event::*, execute, terminal::*};
use tui::{backend::CrosstermBackend, Terminal};

use crate::cursor::*;
use crate::input::*;
use crate::resource::*;

pub const DELTA: u64 = 16;

#[derive(Parser)]
#[command(about, long_about=None)]
struct Refer {
    filename: Vec<String>,
}

pub struct App {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}
impl App {
    pub fn new() -> anyhow::Result<Self> {
        enable_raw_mode().unwrap();

        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(App { terminal })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;

        let mut resource = init_resource()?;

        loop {
            if key_listener(&mut resource)? {
                return Ok(());
            }

            self.terminal.draw(|f| ui::ui(f, &resource))?;
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
        )
        .unwrap();
        self.terminal.show_cursor().unwrap();
    }
}

fn key_listener(res: &mut Resource) -> anyhow::Result<bool> {
    if poll(std::time::Duration::from_millis(DELTA))? {
        let event = read()?;
        if quit_listener(&event) {
            return Ok(true);
        }
        match res.get::<EntryBox>().bool() {
            true => write_key_event(event, res),
            false => normal_key_event(event, res),
        }
    }

    Ok(false)
}

fn quit_listener(event: &Event) -> bool {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => return true,
        _ => {}
    }
    false
}

fn normal_key_event(event: Event, res: &mut Resource) {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            res.get_mut::<Pointer>().toggle();
            res.get_mut::<EntryBox>().toggle();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            ..
        }) => res.get_mut::<Pointer>().set_cursor::<Files>(),
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            ..
        }) => res.get_mut::<Pointer>().set_cursor::<View>(),
        _ => {}
    }
}

fn write_key_event(event: Event, res: &mut Resource) {
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => {
            res.get_mut::<Pointer>().toggle();
            res.get_mut::<EntryBox>().clear();
            res.get_mut::<EntryBox>().toggle();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            ..
        }) => {
            let name = res.get_mut::<EntryBox>().take();
            res.get_mut::<FileBuff>().insert(name);
            res.get_mut::<Pointer>().toggle();
            res.get_mut::<EntryBox>().toggle();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            ..
        }) => res.get_mut::<EntryBox>().pop(),
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            ..
        }) => res.get_mut::<EntryBox>().push(c),
        _ => {}
    }
}

fn init_resource() -> anyhow::Result<Resource> {
    let args = Refer::parse();

    let mut resource = Resource::default();
    resource.insert(args.filename);
    resource.insert(Pointer::new());
    resource.insert(EntryBox::new());
    resource.insert(FileBuff::default());

    Ok(resource)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let panic_buff = Arc::new(Mutex::new(String::new()));

    let old_hook = std::panic::take_hook();

    std::panic::set_hook({
        let panic_buff = panic_buff.clone();
        Box::new(move |info| {
            let mut panic_buff = panic_buff.lock().unwrap();
            let msg = match info.payload().downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => match info.payload().downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<dyn Any>",
                },
            };
            panic_buff.push_str(msg);
        })
    });

    let res = std::panic::catch_unwind(|| {
        let mut main = match App::new() {
            Ok(main) => main,
            Err(err) => panic!("Couldn't create App object: {err}"),
        };

        if let Err(err) = main.run() {
            panic!("Ran into issue while running the application: {err}");
        }
    });

    std::panic::set_hook(old_hook);

    match res {
        Ok(res) => res,
        Err(_) => return Err(anyhow::anyhow!("{}", panic_buff.lock().unwrap())),
    }

    Ok(())
}
