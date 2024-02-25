use std::{env, fs, io::{self, stdout,  Write}, process::exit};
use crossterm::{cursor::{self, *}, event::{self, Event, KeyCode}, execute, style::{ResetColor, SetColors, SetForegroundColor}, terminal::{self, disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand
};


const C_HL_EXTENSIONS: [&str; 3] = [".c", ".h", ".cpp"];
const C_HL_PREPROCESS: [&str; 4] = ["#include", "#ifndef", "#define", "extern"];
const C_HL_KEYWORDS: [&str; 15] = ["switch",    "if",      "while",   "for",    "break",
                         "continue",  "return",  "else",    "struct", "union",
                         "typedef",   "static",  "enum",    "class",  "case"];
const C_HL_TYPES: [&str; 8] = ["int", "long", "double", "float", "char",
                                "unsigned", "signed", "void"];
const TAB_LENGTH: u16 = 4;
const SEPARATORS: [char; 10] = [' ', '.', ',', '{', '}', '(', ')', '<', '>', '"'];
#[derive(Default, PartialEq, PartialOrd)]

enum Mode {
    #[default]
    Normal,
    Insert,
    Visual, // Going to implement visual mode later
    Command,
}

struct Erow {
    data: String,
}

impl Erow {
    fn new(s: String) -> Erow {
        Erow {
            data: s,
        }
    }
}

struct EditorConfig {
    mode: Mode,
    cx: u16,
    cy: u16,
    rx: u16,
    rowoff: u16,
    coloff: u16,
    screenrows: u16,
    screencols: u16,
    numrows: u16,
    rows: Vec<Erow>,
    dirty: bool,
    filename: String, 
    status_msg: String,
    command: String,
    motion_count: u16,
}

impl EditorConfig {
    fn new() -> io::Result<Self> {
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
            Mode::Normal => handle_normal(&mut editor_config).unwrap(),
            Mode::Insert => handle_insert(&mut editor_config).unwrap(),
            Mode::Visual => handle_visual(&mut editor_config).unwrap(),
            Mode::Command => handle_command(&mut editor_config).unwrap(),
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
    // set up terminal for writing to screen
    editor_scroll(editor_config);
    execute!(stdout(), 
        terminal::Clear(ClearType::All),
        cursor::Hide,
        cursor::MoveTo(0,0),

    )?;
    let rowoff: usize = editor_config.rowoff as usize;
    for y in 0..editor_config.screenrows as usize{
        // If line is past file end
        if y >= editor_config.numrows.into() {
            stdout().write_all(b"~\r\n")?;
            continue;
        }
        // line numbering
        let cy = editor_config.cy as usize;
        let lineno = if y + rowoff == editor_config.cy.into() {(y + rowoff).to_string()} else {cy.abs_diff(y + rowoff).to_string()};
        let foreground_color = if y + rowoff == editor_config.cy.into() {crossterm::style::Color::Rgb { r: 0x87, g: 0xce, b: 0xeb }} else {crossterm::style::Color::Black};
        let spaces = " ".repeat(5 - lineno.len());
        stdout().execute(SetForegroundColor(foreground_color))?;
        stdout().write_all(format!("{}{} ", spaces, lineno).as_bytes())?;
        stdout().execute(ResetColor)?;

        // syntax highlighting and line output
        for token in editor_config.rows[y + rowoff].data.split_inclusive(SEPARATORS){
            let token_text = &token[0..token.len() - 1];
            let delimiter = token.chars().last().unwrap();
            let mut textcolor = crossterm::style::Color::Rgb { r: 0xff, g: 0xff, b: 0xff };
            if delimiter == '(' {textcolor = crossterm::style::Color::Blue}
            if delimiter == '"' {textcolor = crossterm::style::Color::Yellow}
            if C_HL_KEYWORDS.contains(&token_text) {textcolor = crossterm::style::Color::Magenta}
            if C_HL_TYPES.contains(&token_text) {textcolor = crossterm::style::Color::DarkGreen}
            if C_HL_PREPROCESS.contains(&token_text) {textcolor = crossterm::style::Color::Red}
            stdout().execute(SetForegroundColor(textcolor))?;
            stdout().write_fmt(format_args!("{token_text}"))?;
            stdout().execute(ResetColor)?;
            stdout().write_fmt(format_args!("{delimiter}"))?;
        }
        stdout().write_all(b"\r\n")?; // Write a newline after each line
        stdout().execute(ResetColor)?;
    }

    // write status line and command
    draw_status(editor_config)?;
    if editor_config.mode == Mode::Command {draw_command(editor_config)?}

    // Prevent cx from going past row length
    let rowlen = editor_config.rows[editor_config.cy as usize].data.len() as u16;
    if editor_config.cx > rowlen{
        execute!(stdout(), cursor::MoveTo(rowlen , editor_config.cy))?;
        editor_config.cx = rowlen;
    }

    // Offset from line numbering
    execute!(stdout(), cursor::MoveTo(editor_config.cx + 6, editor_config.cy - editor_config.rowoff))?;
    execute!(stdout(), cursor::Show)?;
    Ok(())
}

fn draw_status(editor_config: &mut EditorConfig) -> io::Result<()> {
    let (mode_color, mode_string) = match editor_config.mode {
        Mode::Normal => (crossterm::style::Color::Blue, "NORMAL"),
        Mode::Insert => (crossterm::style::Color::Green, "INSERT"),
        Mode::Visual => (crossterm::style::Color::Magenta, "VISUAL"),
        Mode::Command => (crossterm::style::Color::Yellow, "COMMAND"),
    };
    execute!(stdout(),
        SavePosition,
        cursor::MoveTo(0, editor_config.screenrows),
        SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(mode_color)}),
    )?;
    stdout().write_all(mode_string.as_bytes())?;
    stdout().execute(SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(crossterm::style::Color::White)}))?;
    stdout().write_all(editor_config.filename.as_bytes())?;
    if editor_config.dirty {
        stdout().write_all(b" [+] ")?;
    }
    stdout().execute(ResetColor)?;
    stdout().write_all(format!(" Row: {}/{} - Screen {}/{} - Col: {}/{}", editor_config.cy, editor_config.numrows, editor_config.cy - editor_config.rowoff, editor_config.screenrows, editor_config.cx, editor_config.rows[editor_config.cy as usize].data.len()).as_bytes())?;
    stdout().write_all(b"\r\n")?; // Write a newline after each line
    stdout().write_all(editor_config.status_msg.as_bytes())?;
    stdout().execute(RestorePosition)?;
    Ok(())
}

fn draw_command(editor_config: &mut EditorConfig) -> io::Result<()>{
    execute!(stdout(),
        cursor::MoveTo(0, editor_config.screenrows + 1),
        Clear(ClearType::CurrentLine),
    )?;
    stdout().write_all(b":")?;
    stdout().write_all(editor_config.command.as_bytes())?;
    let cmdlen = editor_config.command.len() as u16;
    stdout().execute(cursor::MoveTo(1 + cmdlen, editor_config.screenrows + 1))?;
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
    // set_status_message(editor_config, format!("Number of rows: {}", editor_config.numrows))?;
    editor_config.dirty = false;
    editor_config.filename = filename;
    Ok(())
}

fn editor_save(editor_config: &mut EditorConfig) -> io::Result<()>{
    let content = editor_config.rows.iter()
        .map(|s| &s.data)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(editor_config.filename.clone(), content)?;
    editor_config.dirty = false;
    Ok(())
}

fn insert_row(editor_config: &mut EditorConfig, at: u16, s: String) {
    if at > editor_config.numrows {return;}

    let mut new_row = Erow::new(s);
    editor_config.rows.insert(at.into(), new_row);
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
                            editor_config.mode = Mode::Insert;
                        }
                        'A' => {
                            editor_config.cx = curr_row.data.len() as u16;
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::Insert;

                        }
                        'G' => {
                            editor_config.cy = editor_config.numrows - 1;
                        }
                        'h' => {
                            if editor_config.cx > 0 {editor_config.cx -= 1;}
                        }
                        'i' => {
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::Insert;
                        }
                        'j' => {
                            if editor_config.cy < editor_config.numrows - 1 {editor_config.cy += 1;}
                        }
                        'k' => {
                            if editor_config.cy > 0 {editor_config.cy -= 1;}
                        }
                        'l' => {
                            if editor_config.cx < curr_row.data.len() as u16 {editor_config.cx += 1;}
                        }
                        'o' => {
                            editor_config.cy += 1;
                            editor_config.rows.insert(editor_config.cy as usize, Erow::new(String::new()));
                            editor_config.numrows += 1;
                            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                            editor_config.mode = Mode::Insert;
                        }
                        'w' => {
                            let mut sep = false;
                            while !sep {
                                if editor_config.cx as usize == curr_row.data.len() && editor_config.cy == editor_config.numrows - 1 {
                                   sep = true; 
                                } else if editor_config.cx as usize == curr_row.data.len() {
                                    editor_config.cx = 0;
                                    editor_config.cy += 1;
                                    curr_row = &mut editor_config.rows[editor_config.cy as usize];
                                    if editor_config.cx  < curr_row.data.len() as u16 && !(SEPARATORS.contains(&curr_row.data.chars().nth(editor_config.cx as usize).unwrap())){
                                        sep = true;

                                    }
                                } else if SEPARATORS.contains(&curr_row.data.chars().nth(editor_config.cx as usize).unwrap()) {
                                    while editor_config.cx < curr_row.data.len() as u16 && SEPARATORS.contains(&curr_row.data.chars().nth(editor_config.cx as usize).unwrap()) {
                                        editor_config.cx += 1;
                                    }
                                    sep = true;
                                }
                                else {
                                    editor_config.cx += 1;
                                }
                            }
                        }
                        ':' => {
                            editor_config.mode = Mode::Command;
                            execute!(stdout(),
                                SavePosition,
                                cursor::MoveTo(1, editor_config.numrows),
                            )?;
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
                editor_config.rows[cy].data.insert(editor_config.cx.into(), c);
                editor_config.cx += 1;
            }
            if key.code == KeyCode::Tab {
                let spaces = " ".repeat(TAB_LENGTH.into());
                let cy: usize = editor_config.cy.into();
                editor_config.rows[cy].data.insert_str(editor_config.cx.into(), &spaces);
                stdout().write_all(spaces.as_bytes())?;
                editor_config.cx += TAB_LENGTH;
            }
            if key.code == KeyCode::Esc {
                if editor_config.cx > 0 {editor_config.cx -= 1;}
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                editor_config.mode = Mode::Normal;
            }
            if key.code == KeyCode::Enter {
                insert_row(editor_config, editor_config.cy+1, String::new());
                editor_config.cy += 1;
                editor_config.cx = 0;
            }
            if key.code == KeyCode::Backspace {
                let cy: usize = editor_config.cy.into();
                let len = editor_config.rows[cy].data.len() as u16;
                if editor_config.cx <= len && editor_config.cx > 0{
                    editor_config.rows[cy].data.remove((editor_config.cx - 1).into());
                    editor_config.cx -= 1;
                } else if editor_config.cx == 0 && editor_config.cy > 0 {
                    // delete the current line
                    let cur_str = editor_config.rows[cy].data.clone();
                    let new_cx = editor_config.rows[cy - 1].data.len() as u16;
                    editor_config.rows[cy - 1].data.push_str(&cur_str);
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
                editor_config.mode = Mode::Normal;
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
                editor_config.mode = Mode::Normal;
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
                        set_status_message(editor_config, format!("{} {}L written", editor_config.filename, editor_config.numrows))?;
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
                editor_config.mode = Mode::Normal;
            }
            return Ok(true)
        }
    }
    Ok(false)
}

