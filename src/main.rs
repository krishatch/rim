use core::panic;
use std::{env, fmt::Debug, fs, io::{self, stdout, Stdout, Write}, process::exit};
use crossterm::{cursor::{self, RestorePosition, SavePosition}, event::{self, Event, KeyCode}, execute, style::{ResetColor, SetColors, SetForegroundColor}, terminal::{self, disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand
};
use ratatui::{prelude::*, widgets::*};

const TAB_LENGTH: u16 = 4;
const SEPARATORS: [char; 7] = [' ', '.', ',', '(', ')', '<', '>'];
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
    command: String,
    motion_count: u16,
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
            command: String::default(),
            motion_count: 1,
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
    if editor_config.mode == Mode::COMMAND {
        draw_command(editor_config)?;
        return Ok(());
    }
    editor_scroll(editor_config);
    editor_config.terminal.clear()?;
    editor_config.terminal.hide_cursor()?;
    editor_config.terminal.set_cursor(0, 0)?;
    let rowoff: usize = editor_config.rowoff.into();
    for y in 0..editor_config.screenrows as usize{
        // line numbering
        if y >= editor_config.numrows.into() {break;}
        let lineno = (y + rowoff).to_string();
        let cy = usize::from(editor_config.cy);
        let lineoff = cy.abs_diff(y + rowoff).to_string();
        if y + rowoff == editor_config.cy.into() {
            let spaces = " ".repeat(5 - lineno.len());
            stdout().execute(SetForegroundColor(crossterm::style::Color::Rgb { r: 0x87, g: 0xce, b: 0xeb }))?;
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
    stdout().execute(SavePosition)?;
    editor_config.terminal.set_cursor(0, editor_config.screenrows)?;
    stdout().execute(SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(crossterm::style::Color::White)}))?;
    stdout().write_all(editor_config.filename.as_bytes())?;
    if editor_config.dirty {
        stdout().write_all(b" [+] ")?;
    }
    stdout().execute(ResetColor)?;
    stdout().write_all(format!(" Row: {}/{} - Screen {}/{} - Col: {}/{}", editor_config.cy, editor_config.numrows, editor_config.cy - editor_config.rowoff, editor_config.screenrows, editor_config.cx, editor_config.rows[editor_config.cy as usize].len()).as_bytes())?;
    stdout().write_all(b"\r\n")?; // Write a newline after each line
    stdout().write_all(editor_config.status_msg.as_bytes())?;
    stdout().execute(RestorePosition)?;
    Ok(())
}

fn draw_command(editor_config: &mut EditorConfig) -> io::Result<()>{
    editor_config.terminal.set_cursor(0, editor_config.screenrows + 1)?;
    stdout().execute(Clear(ClearType::CurrentLine))?;
    stdout().write_all(b":")?;
    stdout().write_all(editor_config.command.as_bytes())?;
    let cmdlen = editor_config.command.len() as u16;
    editor_config.terminal.set_cursor(1 + cmdlen, editor_config.screenrows + 1)?;
    Ok(())
}

fn set_status_message(editor_config: &mut EditorConfig, message: String) -> io::Result<()> {
    editor_config.status_msg = message;
    Ok(())
}

fn editor_open(editor_config: &mut EditorConfig, filename: String) -> io::Result<()>{
    let file: String = match fs::read_to_string(filename.clone()){
        Ok(file_content) => file_content,
        Err(_) => {
            let mut file = fs::File::create(&filename)?;
            let empty_file = "\r\n".repeat(editor_config.numrows.into());
            file.write_all(empty_file.as_bytes())?; // Write an empty string to create the file
            insert_row(editor_config, 0, String::default());
            set_status_message(editor_config, String::from("new file"))?;
            String::new()
        }
    };

    for line in file.lines(){
        insert_row(editor_config, editor_config.numrows, line.to_string());
    }
    if editor_config.numrows == 0 {insert_row(editor_config, 0, String::new())}
    set_status_message(editor_config, format!("Number of rows: {}", editor_config.numrows))?;
    editor_config.dirty = false;
    editor_config.filename = filename;
    Ok(())
}

fn editor_save(editor_config: &mut EditorConfig) -> io::Result<()>{
    let content = editor_config.rows.join("\n");
    fs::write(editor_config.filename.clone(), content)?;
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
                for _i in 0..editor_config.motion_count{
                    let mut curr_row = &mut editor_config.rows[editor_config.cy as usize];
                    match c {
                        'a' => {
                            editor_config.cx += 1;
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::INSERT;
                        }
                        'A' => {
                            editor_config.cx = curr_row.len() as u16;
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::INSERT;

                        }
                        'h' => {
                            if editor_config.cx > 0 {editor_config.cx -= 1;}
                        }
                        'i' => {
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::INSERT;
                        }
                        'j' => {
                            if editor_config.cy < editor_config.numrows - 1 {editor_config.cy += 1;}
                        }
                        'k' => {
                            if editor_config.cy > 0 {editor_config.cy -= 1;}
                        }
                        'l' => {
                            if usize::from(editor_config.cx) < curr_row.len() - 1 {editor_config.cx += 1;}
                        }
                        'o' => {
                            editor_config.cy += 1;
                            editor_config.rows.insert(editor_config.cy.into(), String::from(""));
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::INSERT;
                        }
                        'w' => {
                            let mut sep = false;
                            while !sep {
                                if editor_config.cx as usize == curr_row.len() {
                                    editor_config.cx = 0;
                                    editor_config.cy += 1;
                                    curr_row = &mut editor_config.rows[editor_config.cy as usize];
                                } else if SEPARATORS.contains(&curr_row.chars().nth(editor_config.cx as usize).unwrap()) {
                                    sep = true;
                                }
                                else {
                                    editor_config.cx += 1;
                                }
                            }
                            editor_config.cx += 1;
                        }
                        ':' => {
                            editor_config.mode = Mode::COMMAND;
                            stdout().execute(SavePosition)?;
                            editor_config.terminal.set_cursor(1, editor_config.numrows)?;
                        }
                        '0'..='9' => {
                            let num = c.to_digit(10).map(|n| n as u16).unwrap_or(0);
                            if editor_config.motion_count == 1 {editor_config.motion_count = num;}
                            else{
                                editor_config.motion_count *= 10;
                                editor_config.motion_count += num;
                            }
                            set_status_message(editor_config, editor_config.motion_count.to_string())?;
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
                editor_config.motion_count = 1;
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
            if key.code == KeyCode::Tab {
                let spaces = " ".repeat(TAB_LENGTH.into());
                let cy: usize = editor_config.cy.into();
                editor_config.rows[cy].insert_str(editor_config.cx.into(), &spaces);
                stdout().write_all(spaces.as_bytes())?;
                editor_config.cx += TAB_LENGTH;
            }
            if key.code == KeyCode::Esc {
                if editor_config.cx > 0 {editor_config.cx -= 1;}
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                editor_config.mode = Mode::NORMAL;
            }
            if key.code == KeyCode::Enter {
                insert_row(editor_config, editor_config.cy+1, String::new());
                editor_config.cy += 1;
                editor_config.cx = 0;
            }
            if key.code == KeyCode::Backspace {
                let cy: usize = editor_config.cy.into();
                let len = editor_config.rows[cy].len() as u16;
                if editor_config.cx <= len && editor_config.cx > 0{
                    editor_config.rows[cy].remove((editor_config.cx - 1).into());
                    editor_config.cx -= 1;
                } else if editor_config.cx == 0 && editor_config.cy > 0 {
                    // delete the current line
                    let cur_str = editor_config.rows[cy].clone();
                    let new_cx = editor_config.rows[cy - 1].len() as u16;
                    editor_config.rows[cy - 1].push_str(&cur_str);
                    editor_config.rows.remove(cy);
                    editor_config.cy -= 1;
                    editor_config.cx = new_cx;
                    editor_config.numrows -= 1;
                }
            }
            editor_config.dirty = true;
            return Ok(true)
        }
    }
    Ok(false)
}

fn handle_visual(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(50))?{
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                editor_config.mode = Mode::NORMAL;
            }
            editor_config.dirty = true;
            return Ok(true)
        }
    }
    Ok(false)
}

fn handle_command(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(50))?{
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                editor_config.command = String::default();
                stdout().execute(RestorePosition)?;
                editor_config.mode = Mode::NORMAL;
            }
            if let KeyCode::Char(c) = key.code {
                editor_config.command.push(c);
            }
            if key.code == KeyCode::Backspace {
                editor_config.command.pop();
            }
            if key.code == KeyCode::Enter {
                match editor_config.command.as_str() {
                    "w" => {
                        editor_save(editor_config)?;
                        stdout().execute(RestorePosition)?;
                    }
                    "q" => {
                        if !(editor_config.dirty) {
                            disable_raw_mode()?;
                            stdout().execute(LeaveAlternateScreen)?;
                            exit(0);
                        } else {
                            set_status_message(editor_config, String::from("FILE HAS NOT BEEN SAVED!"))?;
                            stdout().execute(RestorePosition)?;
                        }
                    }
                    "wq" => {
                        editor_save(editor_config)?;
                        disable_raw_mode()?;
                        stdout().execute(LeaveAlternateScreen)?;
                        exit(0);
                    }
                    "q!" => {
                        disable_raw_mode()?;
                        stdout().execute(LeaveAlternateScreen)?;
                        exit(0);
                    }
                    _ => {
                        stdout().execute(RestorePosition)?;
                    }
                }
                editor_config.command = String::default();
                editor_config.mode = Mode::NORMAL;
            }
            return Ok(true)
        }
    }
    Ok(false)
}

