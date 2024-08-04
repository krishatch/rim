use std::{env, fmt, fs, io::{self, stdout,  Write}, process::exit};
use crossterm::{cursor::{self, *}, 
    event::{self, Event, KeyCode}, 
    queue,
    execute, 
    style::{ResetColor, SetColors, SetForegroundColor}, 
    terminal::{self, disable_raw_mode, enable_raw_mode, size, Clear, ClearType, DisableLineWrap, EnterAlternateScreen, LeaveAlternateScreen}, 
    ExecutableCommand};

/*** some other packages thay may be useful ***/

// const C_EXTENSIONS: [&str; 3] = [".c", ".h", ".cpp"];
const C_HL_PREPROCESS: [&str; 4] = ["#include", "#ifndef", "#define", "extern"];
const C_HL_KEYWORDS: [&str; 15] = ["switch",    "if",      "while",   "for",    "break",
                         "continue",  "return",  "else",    "struct", "union",
                         "typedef",   "static",  "enum",    "class",  "case"];
const C_HL_TYPES: [&str; 8] = ["int", "long", "double", "float", "char",
                                "unsigned", "signed", "void"];

// const RUST_EXTENSIONS: [&str; 1] = [".rs"];
const RS_DECLARATIONS: [&str; 2] = ["let", "mut"];
const RUST_PREPROCESS: [&str; 1] = [".use"];
const RUST_KEYWORDS: [&str; 51] = ["as", "break",  "const", "continue", "crate",  "else", "enum",
    "extern", "false",  "fn",       "for",      "if",     "impl",    "in",
    "let",    "loop",   "match",    "mod",      "move",   "mut",     "pub",
    "ref",    "return", "self",     "Self",     "static", "struct",  "super",
    "trait",  "true",   "type",     "unsafe",   "use",    "where",   "while",
    "async",  "await",  "dyn",      "abstract", "become", "box",     "do",
    "final",  "macro",  "override", "priv",     "typeof", "unsized", "virtual",
    "yield",  "try"];
const RUST_TYPES: [&str; 16] = ["i8", "i16", "i32",     "i64",     "i128",  "isize",  "u8",
    "u16",   "u32",   "u64",     "u128",    "usize",  "f32",    "f64",
    "bool",  "char"];

const TAB_LENGTH: u16 = 4;
const SEPARATORS: [char; 11] = ['\t', ' ', '.', ',', '{', '}', '(', ')', '<', '>', '"'];

#[derive(Debug)]
enum MyError {
    Io(std::io::Error),
    ParseInt(std::num::ParseIntError),
    // Add other error types as needed
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MyError::Io(ref err) => write!(f, "IO error: {}", err),
            MyError::ParseInt(ref err) => write!(f, "Parse error: {}", err),
            // Handle other cases accordingly
        }
    }
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            MyError::Io(ref err) => Some(err),
            MyError::ParseInt(ref err) => Some(err),
            // Handle other cases accordingly
        }
    }
}
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
    tabs: u16,
}

impl Erow {
    fn new(s: String) -> Erow {
        Erow {
            data: s,
            tabs: 0,
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
    dirty_rows: Vec<u16>,
    filename: String, 
    status_msg: String,
    command: String,
    motion_count: u16,
    d_flag: bool,
    c_flag: bool,
    j_flag: bool,
    vars: Vec<String>,
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
            dirty_rows: Vec::from_iter(0..rows - 2), // mark all rows dirty at beginning
            filename: String::default(),
            status_msg: String::default(),
            command: String::default(),
            motion_count: 1,
            d_flag: false,
            c_flag: false,
            j_flag: false,
            vars: vec![],
        })
    }
}

fn main() -> io::Result<()> {
    /*** Set up terminal ***/
    enable_raw_mode()?;
    execute!(stdout(),
        EnterAlternateScreen,
        DisableLineWrap
    )?;
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

fn editor_scroll(editor_config: &mut EditorConfig) -> io::Result<()> {
  if editor_config.cy < editor_config.rowoff {
    let scroll_diff = editor_config.rowoff - editor_config.cy;
    queue!(stdout(),
        terminal::ScrollDown(scroll_diff)
    )?;
    editor_config.dirty_rows.extend(0..scroll_diff);
    editor_config.rowoff = editor_config.cy;
  } else if editor_config.cy >= editor_config.rowoff + editor_config.screenrows {
    let mut scroll_diff = editor_config.cy - (editor_config.rowoff + editor_config.screenrows);
    scroll_diff+=1;
    queue!(stdout(),
        terminal::ScrollUp(scroll_diff)
    )?;
    editor_config.dirty_rows.extend((editor_config.screenrows - scroll_diff)..editor_config.screenrows);
    editor_config.rowoff = editor_config.cy - editor_config.screenrows + 1;
  }

  if editor_config.rx < editor_config.coloff {
    editor_config.coloff = editor_config.rx;
  } else if editor_config.rx >= editor_config.coloff + editor_config.screencols {
    editor_config.coloff = editor_config.rx - editor_config.screencols + 1;
  }
    Ok(())
}

fn refresh_screen(editor_config: &mut EditorConfig) -> io::Result<()>{
    // set up terminal for writing to screen
    let _ = editor_scroll(editor_config);
    editor_config.dirty_rows.push(editor_config.cy - editor_config.rowoff);
    queue!(stdout(), 
        cursor::Hide,
        cursor::MoveTo(0, editor_config.numrows + 2),
        terminal::Clear(ClearType::CurrentLine),
    )?;
    let rowoff: usize = editor_config.rowoff.into();
    for y in editor_config.dirty_rows.clone() {
        queue!(stdout(), 
            cursor::MoveTo(0,y),
            terminal::Clear(ClearType::CurrentLine),
        )?;
        // If line is past file end
        if y >= editor_config.numrows {
            queue!(stdout(), crossterm::style::Print("~\r\n"))?;
            continue;
        }
        // line numbering
        let cy = editor_config.cy as usize;
        // Relative line numbering first attempt. doesnt work now
        // let lineno = if (y as usize) + rowoff == editor_config.cy.into() {((y as usize) + rowoff).to_string()} else {cy.abs_diff((y as usize) + rowoff).to_string()};
        let lineno = (y as usize + rowoff).to_string();
        // let foreground_color = if (y as usize) + rowoff == editor_config.cy.into() {crossterm::style::Color::Rgb { r: 0x87, g: 0xce, b: 0xeb }} else {crossterm::style::Color::Black};
        let foreground_color = crossterm::style::Color::Rgb { r: 0x87, g: 0xce, b: 0xeb };
        let tab_str = " ".repeat(5 - lineno.len());
        queue!(stdout(), 
            SetForegroundColor(foreground_color),
            crossterm::style::Print(format!("{}{} ", tab_str, lineno)),
            ResetColor
        )?;
        

        // syntax highlighting and line output
        let (keywords, types, preprocess) = match editor_config.filename.split('.').last().unwrap() {
            "rs" => (RUST_KEYWORDS.to_vec(), RUST_TYPES.to_vec(), RUST_PREPROCESS.to_vec()),
            "c" => (C_HL_KEYWORDS.to_vec(), C_HL_TYPES.to_vec(), C_HL_PREPROCESS.to_vec()),
            "cpp" => (C_HL_KEYWORDS.to_vec(), C_HL_TYPES.to_vec(), C_HL_PREPROCESS.to_vec()),
            "h" => (C_HL_KEYWORDS.to_vec(), C_HL_TYPES.to_vec(), C_HL_PREPROCESS.to_vec()),
            _ => {
                let _ = set_status_message(editor_config, "Filetype not supported for syntax higlighting!".to_string());
                (vec![], vec![], vec![])
            },
        };

        let mut declaration = false;
        for token in editor_config.rows[(y as usize) + rowoff].data.split_inclusive(SEPARATORS){
            let token_text = &token[0..token.len() - 1];
            let delimiter = token.chars().last().unwrap();
            let mut textcolor = crossterm::style::Color::Rgb { r: 0xff, g: 0xff, b: 0xff };
            if delimiter == '(' {textcolor = crossterm::style::Color::Blue}
            if delimiter == '"' {textcolor = crossterm::style::Color::Yellow}
            if declaration {
            if !editor_config.vars.contains(&token_text.to_string()) {editor_config.vars.append(vec![token_text.to_string()].as_mut())};
                textcolor = crossterm::style::Color::Rgb { r: 0xf4, g: 0xb6, b: 0xc2 };
                declaration = false;
            }
            if keywords.contains(&token_text) {textcolor = crossterm::style::Color::Magenta}
            if types.contains(&token_text) {textcolor = crossterm::style::Color::DarkGreen}
            if preprocess.contains(&token_text) {textcolor = crossterm::style::Color::Red}
            queue!(stdout(),
                SetForegroundColor(textcolor),
                crossterm::style::Print(token_text.to_string()),
                ResetColor,
                crossterm::style::Print(delimiter.to_string())
            )?;
            if RS_DECLARATIONS.contains(&token_text){
                declaration = true;

            }
        }
        queue!(stdout(),
            crossterm::style::Print("\r\n"),
            ResetColor
        )?;
    }

    // write status line and command
    draw_status(editor_config)?;
    if editor_config.mode == Mode::Command {draw_command(editor_config)?}

    // Prevent cx from going past row length
    let rowlen = editor_config.rows[editor_config.cy as usize].data.len() as u16;
    if editor_config.cx > rowlen{
        queue!(stdout(), cursor::MoveTo(rowlen , editor_config.cy))?;
        editor_config.cx = rowlen;
    }

    // Offset from line numbering
    queue!(stdout(), 
        cursor::MoveTo(editor_config.cx + 6, editor_config.cy - editor_config.rowoff),
        cursor::Show,
    )?;

    // Set dirty rows to empty
    editor_config.dirty_rows = vec![];
    // Flush the queue to do the refresh
    stdout().flush()?;
    Ok(())
}

fn draw_status(editor_config: &mut EditorConfig) -> io::Result<()> {
    let (mode_color, mode_string) = match editor_config.mode {
        Mode::Normal => (crossterm::style::Color::Blue, "NORMAL"),
        Mode::Insert => (crossterm::style::Color::Green, "INSERT"),
        Mode::Visual => (crossterm::style::Color::Magenta, "VISUAL"),
        Mode::Command => (crossterm::style::Color::Yellow, "COMMAND"),
    };
    queue!(stdout(),
        SavePosition,
        cursor::MoveTo(0, editor_config.screenrows),
        SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(mode_color)}),
        crossterm::style::Print(mode_string),
        SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(crossterm::style::Color::White)}),
        crossterm::style::Print(editor_config.filename.clone()),
    )?;
    if editor_config.dirty {
        queue!(stdout(), crossterm::style::Print(" [+] "))?;
    }
    queue!(stdout(),
        ResetColor,
        crossterm::style::Print(format!(" Row: {}/{} - Screen {}/{} - Col: {}/{} - Rowoff {}", editor_config.cy, editor_config.numrows, editor_config.cy - editor_config.rowoff, editor_config.screenrows, editor_config.cx, editor_config.rows[editor_config.cy as usize].data.len(), editor_config.rowoff)),
        crossterm::style::Print("\r\n"),
        crossterm::style::Print(editor_config.status_msg.clone()),
        RestorePosition
    )?;
    Ok(())
}

fn draw_command(editor_config: &mut EditorConfig) -> io::Result<()>{
    queue!(stdout(),
        cursor::MoveTo(0, editor_config.screenrows + 1),
        Clear(ClearType::CurrentLine),
        crossterm::style::Print(":"),
        crossterm::style::Print(editor_config.command.clone()),
        cursor::MoveTo(1 + editor_config.command.len() as u16, editor_config.screenrows + 1)
    )?;
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

    let new_row = Erow::new(s);
    editor_config.rows.insert(at.into(), new_row);
    editor_config.numrows += 1;
    editor_config.dirty = true;
}

/*** Keyboard Event Handling ***/
fn handle_normal(editor_config: &mut EditorConfig) -> io::Result<bool>  {
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            // mark current row dirty (if we leave this row we rand to make lineno dark!)
            editor_config.dirty_rows.push(editor_config.cy - editor_config.rowoff);
            if let KeyCode::Char(c) = key.code {
                for _i in 0..editor_config.motion_count{
                    let mut cy = editor_config.cy as usize;
                    if editor_config.d_flag {
                        match c {
                            'd' => {
                                if editor_config.numrows == 1 {
                                    editor_config.rows[0].data = String::new();
                                    editor_config.cx = 0;
                                    editor_config.cy = 0;
                                } else {
                                    editor_config.rows.remove(editor_config.cy as usize);
                                    editor_config.numrows -= 1;
                                    if editor_config.cy == editor_config.numrows {editor_config.cy -= 1}
                                }
                            }
                            'w' => {
                                // Do later
                            }
                            _ => {}
                        }
                        editor_config.d_flag = false;
                    } else if editor_config.c_flag{
                        match c {
                            'w' => {
                                // do later
                            }
                            'e' => {
                                // do later
                            }
                            _ => {}
                        }
                        editor_config.c_flag = false;
                    } else {
                        match c {
                            'a' => {
                                editor_config.cx += 1;
                                stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                                editor_config.mode = Mode::Insert;
                            }
                            'A' => {
                                editor_config.cx = editor_config.rows[cy].data.len() as u16;
                                stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                                editor_config.mode = Mode::Insert;

                            }
                            'c' => {
                                editor_config.c_flag = true;
                            }
                            'd' => {
                                editor_config.d_flag = true;
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
                                if editor_config.cx < editor_config.rows[cy].data.len() as u16 {editor_config.cx += 1;}
                            }
                            'o' => {
                                editor_config.cy += 1;
                                editor_config.rows.insert(editor_config.cy as usize, Erow::new(String::new()));
                                editor_config.numrows += 1;
                                stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
                                editor_config.mode = Mode::Insert;
                            }
                            'v' => {
                                editor_config.mode = Mode::Visual;
                            }
                            'w' => {
                                let mut sep = false;
                                while !sep {
                                    if editor_config.cx as usize == editor_config.rows[cy].data.len() && editor_config.cy == editor_config.numrows - 1 {
                                       sep = true; 
                                    } else if editor_config.cx as usize == editor_config.rows[cy].data.len() {
                                        editor_config.cx = 0;
                                        editor_config.cy += 1;
                                        cy = editor_config.cy as usize;
                                        if editor_config.cx  < editor_config.rows[cy].data.len() as u16 && !(SEPARATORS.contains(&editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap())){
                                            sep = true;

                                        }
                                    } else if SEPARATORS.contains(&editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap()) {
                                        while editor_config.cx < editor_config.rows[cy].data.len() as u16 && SEPARATORS.contains(&editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap()) {
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
                }
                editor_config.motion_count = 1;
                return Ok(true);
            }         
        }
    }
    Ok(false)
}

fn auto_indent(editor_config: &mut EditorConfig) -> Result<(), MyError> {
    let current_line = editor_config.rows[editor_config.cy as usize].data.clone();
    let (split_left, split_right) = current_line.split_at(editor_config.cx as usize);

    let leading_spaces = current_line.chars().take_while(|c| *c == ' ').count();
    
    let indent_str = " ".repeat(leading_spaces);

    // Simplified line splitting and insertion
    editor_config.rows.remove(editor_config.cy as usize);
    editor_config.numrows -= 1;
    insert_row(editor_config, editor_config.cy, split_left.to_string());
    insert_row(editor_config, editor_config.cy + 1, format!("{}{}", indent_str, split_right));

    // Calculate indentation for cursor positioning
    let additional_indent = if !split_right.is_empty() && [']', '}', ')'].contains(&split_right.chars().next().unwrap()) {
        // Handle specific closing characters with additional indentation
        let extra_indent_str = " ".repeat(4); // Adjust the number of spaces as needed
        insert_row(editor_config, editor_config.cy + 1, extra_indent_str.clone());
        editor_config.rows[editor_config.cy as usize + 1].data.insert_str(0, &extra_indent_str);
        TAB_LENGTH // Adjust according to your indentation strategy
    } else {
        0
    };

    editor_config.cx = leading_spaces as u16 + additional_indent;
    editor_config.cy += 1;
    Ok(())
}

fn handle_insert(editor_config: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char(c) = key.code {
                let cy: usize = editor_config.cy as usize;
                if editor_config.j_flag && c == 'k' {
                    editor_config.rows[cy].data.remove((editor_config.cx - 1) as usize);
                    editor_config.cx -= 1;
                    editor_config.j_flag = false;
                    stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                    editor_config.mode = Mode::Normal;
                } else {
                    editor_config.rows[cy].data.insert(editor_config.cx.into(), c);
                    editor_config.cx += 1;
                    if c == '{' {
                        editor_config.rows[cy].data.insert(editor_config.cx.into(), '}');
                    }else if c == '(' {
                        editor_config.rows[cy].data.insert(editor_config.cx.into(), ')');
                    }else if c == '[' {
                        editor_config.rows[cy].data.insert(editor_config.cx.into(), ']');
                    } else if editor_config.cx < editor_config.rows[cy].data.len() as u16 && ((c == '}' && editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap() == '}') ||
                    (c == ')' && editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap() == ')') ||
                    (c == ']' && editor_config.rows[cy].data.chars().nth(editor_config.cx as usize).unwrap() == ']')) {
                        editor_config.rows[cy].data.remove(editor_config.cx as usize);
                    } else if c == 'j' {
                        editor_config.j_flag = true;
                    }
                }
            } else if key.code == KeyCode::Tab {
                let tab_str = " ".repeat(TAB_LENGTH as usize);
                let cy: usize = editor_config.cy.into();
                editor_config.rows[cy].data.insert_str(editor_config.cx.into(), &tab_str);
                editor_config.cx += TAB_LENGTH;
            } else if key.code == KeyCode::Esc {
                if editor_config.cx > 0 {editor_config.cx -= 1;}
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                editor_config.mode = Mode::Normal;
            } else if key.code == KeyCode::Enter {
                let _ = auto_indent(editor_config);
            } else if key.code == KeyCode::Backspace {
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
    if event::poll(std::time::Duration::from_millis(1))?{
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
    if event::poll(std::time::Duration::from_millis(1))?{
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

