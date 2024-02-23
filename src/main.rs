use core::panic;
use std::{env, fs, io::{self, stdout, Stdout, Write}, process::exit};
use crossterm::{event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};

#[derive(Default)]
enum Mode {
    #[default]
    NORMAL,
    INSERT,
    VISUAL,
    COMMAND,
}

struct EditorConfig {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    mode: Mode,
    cx: u16,
    cy: u16,
    rx: u16,
    rowoff: u16,
    coloff: u16,
    screenrows: u16,
    screencols: u16,
    numrows: u16,
    rows: Vec<String>,
    dirty: bool,
    filename: String,
    status_msg: String,
    b_wrap: u16,
    v_cx: u16,
    v_cy: u16, 
}

impl EditorConfig {
    fn new() -> io::Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        let (cols, rows) = size()?;

        Ok(EditorConfig {
            mode: Mode::default(),
            cx: 0,
            cy: 0,
            rx: 0,
            rowoff: 0,
            coloff: 0,
            screenrows: rows - 2, // 2 bottom rows are for status line
            screencols: cols,
            numrows: 0,
            rows: vec![],
            dirty: false,
            filename: String::default(),
            status_msg: String::default(),
            b_wrap: 0,
            v_cx: 0,
            v_cy: 0,
            terminal,
        })
    }
}

fn main() -> io::Result<()> {
    /*** Set up terminal ***/
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut editor_config = EditorConfig::new().unwrap();
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 {editor_open(&mut editor_config, args[1].clone()).unwrap();}

    let mut refresh = true;
    loop {
        if refresh {let _ = refresh_screen(&mut editor_config);} 
        
        refresh = match editor_config.mode {
            Mode::NORMAL => handle_normal(&mut editor_config).unwrap(),
            Mode::INSERT => handle_insert().unwrap(),
            Mode::VISUAL => handle_visual().unwrap(),
            Mode::COMMAND => handle_command().unwrap(),
        };
    }
}

fn refresh_screen(editor_config: &mut EditorConfig) -> io::Result<()>{
    editor_config.terminal.hide_cursor()?;
    editor_config.terminal.set_cursor(0, 0)?;
    for line in editor_config.rows.iter(){
        stdout().write_all(line.as_bytes())?;
        stdout().write_all(b"\r\n")?; // Write a newline after each line
    }
    editor_config.terminal.flush()?;
    editor_config.terminal.set_cursor(editor_config.cx, editor_config.cy)?;
    editor_config.terminal.show_cursor()?;
    Ok(())
}


fn editor_open(editor_config: &mut EditorConfig, filename: String) -> io::Result<()>{
    let file: String = match fs::read_to_string(filename.clone()){
        Ok(file_content) => file_content,
        Err(e) => {
            panic!("Error reading file '{}': {}", filename, e);
        }
    };

    for line in file.lines(){
        let mut linelen = line.len();
        while linelen > 0 && (line.chars().nth(linelen - 1) == Some('\n') || line.chars().nth(linelen - 1) == Some('r')) {
            linelen -= 1;
        }
        insert_row(editor_config, editor_config.numrows, line.to_string());
    }
    editor_config.dirty = false;
    Ok(())
}

fn insert_row(editor_config: &mut EditorConfig, at: u16, s: String) {
    if at > editor_config.numrows {return;}

    editor_config.rows.insert(at.into(), s);
    editor_config.numrows += 1;
    editor_config.dirty = true;
}

/*** Keyboard Event Handling ***/
fn handle_normal(editor_config: &mut EditorConfig) -> io::Result<bool>  {
    if event::poll(std::time::Duration::from_millis(50))?{
        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char(c) = key.code {
                match c {
                    'q' => {
                    disable_raw_mode()?;
                    stdout().execute(LeaveAlternateScreen)?;
                    exit(0);
                    }
                    'j' => {
                        if editor_config.cy < editor_config.numrows - 1 {editor_config.cy += 1;}
                        return Ok(true);
                    }
                    'k' => {
                        if editor_config.cy > 0 {editor_config.cy -= 1;}
                        return Ok(true);
                    }
                    _ => {}
                }
            } 
        }
    }
    Ok(false)
}

fn handle_insert() -> io::Result<bool>{ 
    Ok(false)
}

fn handle_visual() -> io::Result<bool>{ 
    Ok(false)
}

fn handle_command() -> io::Result<bool>{ 
    Ok(false)
}
