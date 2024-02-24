use core::panic;
use std::{env, fs, io::{self, stdout, Stdout, Write}, process::exit};
use crossterm::{cursor, event::{self, Event, KeyCode}, execute, style::{ResetColor, SetColors, SetForegroundColor}, terminal::{disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand
};
use ratatui::{prelude::*, widgets::*};
#[derive(Default, PartialEq, PartialOrd)]
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
            Mode::INSERT => handle_insert(&mut editor_config).unwrap(),
            Mode::VISUAL => handle_visual(&mut editor_config).unwrap(),
            Mode::COMMAND => handle_command(&mut editor_config).unwrap(),
        };
    }
}

fn editor_scroll(editor_config: &mut EditorConfig) {
  if editor_config.cy < editor_config.rowoff {
    editor_config.rowoff = editor_config.cy;
  } else if editor_config.cy >= editor_config.rowoff + editor_config.screenrows {
    editor_config.rowoff = editor_config.cy - editor_config.screenrows + 1;
  }

  if editor_config.rx < editor_config.coloff {
    editor_config.coloff = editor_config.rx;
  } else if editor_config.rx >= editor_config.coloff + editor_config.screencols {
    editor_config.coloff = editor_config.rx - editor_config.screencols + 1;
  }
}

fn refresh_screen(editor_config: &mut EditorConfig) -> io::Result<()>{
    editor_scroll(editor_config);
    editor_config.terminal.clear()?;
    editor_config.terminal.hide_cursor()?;
    editor_config.terminal.set_cursor(0, 0)?;
    let rowoff: usize = editor_config.rowoff.into();
    for y in 0..editor_config.screenrows as usize{
        // line numbering
        let lineno = (y + rowoff).to_string();
        let cy = usize::from(editor_config.cy);
        let lineoff = cy.abs_diff(y + rowoff).to_string();
        if y + rowoff == editor_config.cy.into() {
            let spaces = " ".repeat(5 - lineno.len());
            stdout().execute(SetForegroundColor(crossterm::style::Color::Blue))?;
            stdout().write_all(format!("{}{} ", spaces, lineno).as_bytes())?;
            stdout().execute(ResetColor)?;
            stdout().write_all(editor_config.rows[y + rowoff].clone().as_bytes())?;
            stdout().write_all(b"\r\n")?; // Write a newline after each line
        } else {
            let spaces = " ".repeat(5 - lineoff.len());
            stdout().execute(SetForegroundColor(crossterm::style::Color::Black))?;
            stdout().write_all(format!("{}{} ", spaces, lineoff).as_bytes())?;
            stdout().execute(ResetColor)?;
            stdout().write_all(editor_config.rows[y + rowoff].clone().as_bytes())?;
            stdout().write_all(b"\r\n")?; // Write a newline after each line
        }
    }
    // write status line
    draw_status(editor_config)?;
    if editor_config.mode == Mode::COMMAND {
        draw_command(editor_config)?;
    }
    editor_config.terminal.flush()?;
    let row: usize = editor_config.cy.into();
    editor_config.terminal.set_cursor(editor_config.cx + 6, editor_config.cy - editor_config.rowoff)?;
    if usize::from(editor_config.cx) > editor_config.rows[row].len() {
        editor_config.terminal.set_cursor(editor_config.rows[row].len().try_into().unwrap(), editor_config.cy)?;
        editor_config.cx = <usize as TryInto<u16>>::try_into(editor_config.rows[row].len()).unwrap();
    }
    editor_config.terminal.show_cursor()?;
    Ok(())
}

fn draw_status(editor_config: &mut EditorConfig) -> io::Result<()> {
    stdout().execute(SetForegroundColor(crossterm::style::Color::Red))?;
    stdout().execute(SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(crossterm::style::Color::White)}))?;
    stdout().write_all(editor_config.filename.as_bytes())?;
    stdout().execute(ResetColor)?;
    stdout().write_all(format!(" Row: {}/{} - Screen {}/{} - Col: {}", editor_config.cy, editor_config.numrows, editor_config.cy - editor_config.rowoff, editor_config.screenrows, editor_config.cx).as_bytes())?;
    stdout().write_all(b"\r\n")?; // Write a newline after each line
    Ok(())
}

fn draw_command(editor_config: &mut EditorConfig) -> io::Result<()>{
    stdout().write_all(b":")?;

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
    editor_config.filename = filename;
    Ok(())
}

fn editor_save(editor_config: &mut EditorConfig) -> io::Result<()>{
    let content = editor_config.rows.join("\n");
    fs::write(editor_config.filename.clone(), content)?;
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
                    'h' => {
                        if editor_config.cx > 0 {editor_config.cx -= 1;}
                    }
                    'j' => {
                        if editor_config.cy < editor_config.numrows - 1 {editor_config.cy += 1;}
                    }
                    'k' => {
                        if editor_config.cy > 0 {editor_config.cy -= 1;}
                    }
                    'l' => {
                        let row: usize = editor_config.cy.into();
                        if usize::from(editor_config.cx) < editor_config.rows[row].len() - 1 {editor_config.cx += 1;}
                    }
                    'w' =>{
                        editor_save(editor_config)?;
                    }
                    'i' => {
                        stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                        editor_config.mode = Mode::INSERT;
                    }
                    'a' => {
                        editor_config.cx += 1;
                        stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                        editor_config.mode = Mode::INSERT;
                    }
                    'o' => {
                        editor_config.cy += 1;
                        editor_config.rows.insert(editor_config.cy.into(), String::from(""));
                        stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                        editor_config.mode = Mode::INSERT;
                    }
                    _ => {}
                }
                return Ok(true);
            } 
        }
    }
    Ok(false)
}

fn handle_insert(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(50))?{
        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char(c) = key.code {
                stdout().write_all(&[c as u8])?;
                let cy: usize = editor_config.cy.into();
                editor_config.rows[cy].insert(editor_config.cx.into(), c);
                editor_config.cx += 1;
            }
            if key.code == KeyCode::Esc {
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                editor_config.mode = Mode::NORMAL;
            }
            return Ok(true)
        }
    }
    Ok(false)
}

fn handle_visual(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    Ok(false)
}

fn handle_command(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    Ok(false)
}

