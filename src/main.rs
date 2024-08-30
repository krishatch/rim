use std::{env, fs, io::{self, stdout,  Write}, process::exit};
use crossterm::{cursor::{self, *}, event::{self, Event, KeyCode}, execute, queue, style::{Color, ResetColor, SetColors, SetForegroundColor}, terminal::{
        self, 
        disable_raw_mode, 
        enable_raw_mode, 
        size, 
        Clear, 
        ClearType, 
        DisableLineWrap, 
        EnterAlternateScreen, 
        LeaveAlternateScreen
    }, ExecutableCommand};

// C Syntax Highlighting
const C_PREPROCESS: [&str; 4] = ["#include", "#ifndef", "#define", "extern"];
const C_KEYWORDS: [&str; 15] = ["switch",    "if",      "while",   "for",    "break",
                         "continue",  "return",  "else",    "struct", "union",
                         "typedef",   "static",  "enum",    "class",  "case"];
const C_TYPES: [&str; 8] = ["int", "long", "double", "float", "char",
                                "unsigned", "signed", "void"];
const C_ENCLOSERS: [char; 2] = ['<', '"'];

// Rust Syntax Highlighting
const RUST_PREPROCESS: [&str; 1] = [".use"];
const RUST_DECLARATIONS: [&str; 2] = ["let", "mut"];
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

// Lua Syntax Highlighting
const LUA_PREPROCCESS: [&str; 11] = ["priority", "prefsys", "identifier", "class", "handler",
    "hide", "defer", "disallow_manual", "import", "version", "description"];
const LUA_KEYWORDS: [&str; 21] = ["and", "break", "do", "else", "elseif", "end", "false",
    "for", "function", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true",
    "until", "while"];
const LUA_TYPES: [&str; 1] = ["local"];
const LUA_ENCLOSERS: [char; 2] = ['"', '\''];

const TAB_LENGTH: usize = 4;
const SEPARATORS: [char; 12] = [';', '\t', ' ', '.', ',', '{', '}', '(', ')', '<', '>', '"'];

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
    indent: usize,
}

impl Erow {
    fn new(s: String) -> Erow {
        Erow {
            data: s,
            indent: 0,
        }
    }
}

struct EditorConfig {
    mode: Mode,
    cx: usize,
    cy: usize,
    rx: usize,
    rowoff: usize,
    coloff: usize,
    screenrows: usize,
    screencols: usize,
    numrows: usize,
    rows: Vec<Erow>,
    dirty: bool,
    dirty_rows: Vec<usize>,
    filename: String, 
    status_msg: String,
    command: String,
    motion: String,
    motion_count: usize,
    vars: Vec<String>,
    j_flag: bool,
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
            screenrows: rows as usize - 2, // 2 bottom rows are for status line
            screencols: cols as usize,
            numrows: 0,
            rows: vec![],
            dirty: false,
            dirty_rows: Vec::from_iter(0..rows as usize - 2), // mark all rows dirty at beginning
            filename: String::default(),
            status_msg: String::default(),
            command: String::default(),
            motion: String::default(),
            motion_count: 1,
            vars: vec![],
            j_flag: false,
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
    let mut ec = EditorConfig::new().unwrap();
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 {editor_open(&mut ec, args[1].clone()).unwrap();}

    let mut refresh = true;
    loop {
        if refresh {let _ = refresh_screen(&mut ec);} 
        
        refresh = match ec.mode {
            Mode::Normal => handle_normal(&mut ec).unwrap(),
            Mode::Insert => handle_insert(&mut ec).unwrap(),
            Mode::Visual => handle_visual(&mut ec).unwrap(),
            Mode::Command => handle_command(&mut ec).unwrap(),
        };
    }
}

fn editor_scroll(ec: &mut EditorConfig) -> io::Result<()> {
  if ec.cy < ec.rowoff {
    let mut scroll_diff = ec.rowoff - ec.cy;
    scroll_diff = if scroll_diff > ec.screenrows {ec.screenrows} else {scroll_diff};
    queue!(stdout(),
        terminal::ScrollDown(scroll_diff as u16)
    )?;
    ec.dirty_rows.extend(0..scroll_diff);
    ec.rowoff = ec.cy;
  } else if ec.cy >= ec.rowoff + ec.screenrows {
    let mut scroll_diff = ec.cy - (ec.rowoff + ec.screenrows);
    scroll_diff+=1;
    scroll_diff = if scroll_diff > ec.screenrows {ec.screenrows} else {scroll_diff};
    queue!(stdout(),
        terminal::ScrollUp(scroll_diff as u16)
    )?;
    ec.dirty_rows.extend((ec.screenrows - scroll_diff)..ec.screenrows);
    ec.rowoff = ec.cy - ec.screenrows + 1;
  }

  if ec.rx < ec.coloff {
    ec.coloff = ec.rx;
  } else if ec.rx >= ec.coloff + ec.screencols {
    ec.coloff = ec.rx - ec.screencols + 1;
  }
    Ok(())
}

fn refresh_screen(ec: &mut EditorConfig) -> io::Result<()>{
    // set up terminal for writing to screen
    let _ = editor_scroll(ec);
    ec.dirty_rows.push(ec.cy - ec.rowoff);
    queue!(stdout(), 
        cursor::Hide,
        cursor::MoveTo(0, ec.numrows as u16 + 2),
        terminal::Clear(ClearType::CurrentLine),
    )?;

    let rowoff: usize = ec.rowoff;
    for y in ec.dirty_rows.clone() {
        queue!(stdout(), 
            cursor::MoveTo(0,y as u16),
            terminal::Clear(ClearType::CurrentLine),
        )?;

        // If line is past file end draw ~
        if y >= ec.numrows {
            queue!(stdout(), crossterm::style::Print("~\r\n"))?;
            continue;
        }

        // line numbering
        // Relative line numbering first attempt. doesnt work now
        let lineno = (y + rowoff).to_string();
        let foreground_color = crossterm::style::Color::Rgb { r: 0x87, g: 0xce, b: 0xeb };
        let lineno_spaces = " ".repeat(5 - lineno.len());
        queue!(stdout(), 
            SetForegroundColor(foreground_color),
            crossterm::style::Print(format!("{}{} ", lineno_spaces, lineno)),
            ResetColor
        )?;
        

        // highlighted words
        let (keywords, types, preprocess, enclosers) = match ec.filename.split('.').last().unwrap() {
            "rs" => (RUST_KEYWORDS.to_vec(), RUST_TYPES.to_vec(), RUST_PREPROCESS.to_vec(), vec![]),
            "c" => (C_KEYWORDS.to_vec(), C_TYPES.to_vec(), C_PREPROCESS.to_vec(), C_ENCLOSERS.to_vec()),
            "cpp" => (C_KEYWORDS.to_vec(), C_TYPES.to_vec(), C_PREPROCESS.to_vec(), C_ENCLOSERS.to_vec()),
            "h" => (C_KEYWORDS.to_vec(), C_TYPES.to_vec(), C_PREPROCESS.to_vec(), C_ENCLOSERS.to_vec()),
            "lua" => (LUA_KEYWORDS.to_vec(), LUA_TYPES.to_vec(), LUA_PREPROCCESS.to_vec(), LUA_ENCLOSERS.to_vec()),
            _ => {
                let _ = set_status_message(ec, "Filetype not supported for syntax higlighting!".to_string());
                (vec![], vec![], vec![], vec![])
            },
        };

        let mut enclosed = false;
        for token in ec.rows[y + rowoff].data.split_inclusive(SEPARATORS){
            // Default white
            let mut textcolor = crossterm::style::Color::Rgb { r: 0xff, g: 0xff, b: 0xff };

            let (mut token_text, mut separator);
            if SEPARATORS.contains(&token.chars().last().unwrap()){
                token_text = token.split(SEPARATORS).next().unwrap();
                separator = token.chars().last().unwrap().to_string();

            } else {
                token_text = token;
                separator = "".to_string();
            }

            // highlight token text
            if separator == '('.to_string() {textcolor = crossterm::style::Color::Blue}
            if token_text != "".to_string() && token_text.chars().next().unwrap().is_numeric() {textcolor = crossterm::style::Color::Red}
            if keywords.contains(&token_text) {textcolor = crossterm::style::Color::Magenta}
            if types.contains(&token_text) {textcolor = crossterm::style::Color::DarkGreen}
            if preprocess.contains(&token_text) {textcolor = crossterm::style::Color::Red}

            // If we are in an "encloser" (like "") make all highlights yellow
            if enclosed {
                token_text = token;
                separator = "".to_string();
                textcolor = crossterm::style::Color::Yellow;
                if ['\'', '\"', '>'].contains(&token.chars().last().unwrap()){
                    enclosed = false
                }
            }
            if enclosers.contains(&token.chars().next().unwrap()) {
                token_text = token;
                separator = "".to_string();
                textcolor = crossterm::style::Color::Yellow;
                enclosed = true;
            }

            queue!(stdout(),
                SetForegroundColor(textcolor),
                crossterm::style::Print(token_text.to_string()),
                )?;
            queue!(stdout(),
                ResetColor,
                crossterm::style::Print(separator.to_string())
            )?;
        }

        queue!(stdout(),
            crossterm::style::Print("\r\n"),
            ResetColor
        )?;
    }

    // write status line and command
    draw_status(ec)?;
    if ec.mode == Mode::Command {draw_command(ec)?}

    // Prevent cx from going past row length
    let rowlen = ec.rows[ec.cy].data.len();
    if ec.cx > rowlen{
        queue!(stdout(), cursor::MoveTo(rowlen as u16, ec.cy as u16))?;
        ec.cx = rowlen;
    }

    // Offset from line numbering
    queue!(stdout(), 
        cursor::MoveTo(ec.cx as u16 + 6, ec.cy as u16 - ec.rowoff as u16),
        cursor::Show,
    )?;

    // Set dirty rows to empty
    ec.dirty_rows = vec![];
    // Flush the queue to do the refresh
    stdout().flush()?;
    Ok(())
}

fn draw_status(ec: &mut EditorConfig) -> io::Result<()> {
    let (mode_color, mode_string) = match ec.mode {
        Mode::Normal => (crossterm::style::Color::Blue, "NORMAL"),
        Mode::Insert => (crossterm::style::Color::Green, "INSERT"),
        Mode::Visual => (crossterm::style::Color::Magenta, "VISUAL"),
        Mode::Command => (crossterm::style::Color::Yellow, "COMMAND"),
    };
    queue!(stdout(),
        SavePosition,
        cursor::MoveTo(0, ec.screenrows as u16),
        SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(mode_color)}),
        crossterm::style::Print(mode_string),
        SetColors(crossterm::style::Colors{ foreground: Some(crossterm::style::Color::Black), background: Some(crossterm::style::Color::White)}),
        crossterm::style::Print(ec.filename.clone()),
    )?;
    if ec.dirty {
        queue!(stdout(), crossterm::style::Print(" [+] "))?;
    }
    queue!(stdout(),
        ResetColor,
        crossterm::style::Print(
            format!(
                " Row: {}/{} - Screen {}/{} - Col: {}/{} - Rowoff {}", 
                ec.cy, 
                ec.numrows, 
                ec.cy - ec.rowoff, 
                ec.screenrows, 
                ec.cx, 
                ec.rows[ec.cy].data.len(), 
                ec.rowoff
            )
        ),
        crossterm::style::Print("\r\n"),
        crossterm::style::Print(ec.status_msg.clone()),
        RestorePosition
    )?;
    Ok(())
}

fn draw_command(ec: &mut EditorConfig) -> io::Result<()>{
    queue!(stdout(),
        cursor::MoveTo(0, ec.screenrows as u16 + 1),
        Clear(ClearType::CurrentLine),
        crossterm::style::Print(":"),
        crossterm::style::Print(ec.command.clone()),
        cursor::MoveTo(1 + ec.command.len() as u16, ec.screenrows as u16 + 1)
    )?;
    Ok(())
}

fn set_status_message(ec: &mut EditorConfig, message: String) -> io::Result<()> {
    ec.status_msg = message;
    Ok(())
}

fn editor_open(ec: &mut EditorConfig, filename: String) -> io::Result<()>{
    let file: String = match fs::read_to_string(filename.clone()){
        Ok(file_content) => file_content,
        Err(_) => {
            let mut file = fs::File::create(&filename)?;
            let empty_file = "\r\n".repeat(ec.numrows);
            file.write_all(empty_file.as_bytes())?; // Write an empty string to create the file
            insert_row(ec, 0, String::default());
            set_status_message(ec, String::from("new file"))?;
            String::new()
        }
    };

    for line in file.lines(){
        insert_row(ec, ec.numrows, line.to_string());
    }
    if ec.numrows == 0 {insert_row(ec, 0, String::new())}
    ec.dirty = false;
    ec.filename = filename;
    Ok(())
}

fn editor_save(ec: &mut EditorConfig) -> io::Result<()>{
    let content = ec.rows.iter()
        .map(|s| &s.data)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(ec.filename.clone(), content)?;
    ec.dirty = false;
    Ok(())
}

fn insert_row(ec: &mut EditorConfig, at: usize, s: String) {
    if at > ec.numrows {return;}

    let new_row = Erow::new(s);
    ec.rows.insert(at, new_row);
    ec.numrows += 1;
    ec.dirty = true;
}

/*** Motions ***/

fn colon(ec: &mut EditorConfig){
    ec.mode = Mode::Command;
    let _ = execute!(stdout(),
        SavePosition,
        cursor::MoveTo(1, ec.numrows as u16),
    );
}

fn ua_motion(ec: &mut EditorConfig){
    ec.cx = ec.rows[ec.cy].data.len();
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn a_motion(ec: &mut EditorConfig) {
    ec.cx += 1;
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn o_motion(ec: &mut EditorConfig){
    // Insert a new row with the same indention as the current row
    let indents = ec.rows[ec.cy].indent;
    let leading_spaces = " ".repeat(indents * TAB_LENGTH);
    ec.cy += 1;
    ec.rows.insert(ec.cy, Erow::new(leading_spaces));
    ec.rows[ec.cy].indent = indents;
    ec.numrows += 1;
    // set all rows after as dirty
    ec.dirty_rows.extend((ec.cy - ec.rowoff)..ec.screenrows);
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn uo_motion(ec: &mut EditorConfig){
    // Insert a new row with the same indention as the current row
    let indents = ec.rows[ec.cy].indent;
    let leading_spaces = " ".repeat(indents * TAB_LENGTH);
    ec.rows.insert(ec.cy, Erow::new(leading_spaces));
    ec.rows[ec.cy].indent = indents;
    ec.numrows += 1;
    // set all rows after as dirty
    ec.dirty_rows.extend((ec.cy - ec.rowoff)..ec.screenrows);
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn w_motion(ec: &mut EditorConfig){
    // Move forward 1 (make sure we dont go past eof)
    ec.cx += 1;
    if ec.cy == ec.numrows - 1 && ec.cx >= ec.rows[ec.cy].data.len() {return}

    if ec.cx >= ec.rows[ec.cy].data.len() {
        ec.cy += 1;
        ec.cx = 0;
        if ec.rows[ec.cy].data.is_empty() {return}
        while ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap() == ' ' {ec.cx += 1}
        return
    }

    // Find a separator
    while !SEPARATORS.contains(&ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap()){
        if ec.cy == ec.numrows && ec.cx >= ec.rows[ec.cy].data.len() {return}
        if ec.cx == ec.rows[ec.cy].data.len() - 1 {return}
        ec.cx += 1;
    }

    // Find start of next token
    while SEPARATORS.contains(&ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap()){
        if ec.cx == ec.rows[ec.cy].data.len() - 1 {return}
        ec.cx += 1;
    }
}

fn b_motion(ec: &mut EditorConfig){
    // Move back 1 (make sure we dont go below 0)
    if ec.cx == 0 && ec.cy == 0 {return}
    while ec.cx == 0 {
        ec.cy -= 1;
        ec.cx = ec.rows[ec.cy].data.len();
    }
    ec.cx -= 1;

    // Keep going back until we find a letter
    while SEPARATORS.contains(&ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap()){
        if ec.cx == 0 && ec.cy == 0 {return}
        while ec.cx == 0 {
            ec.cy -= 1;
            ec.cx = ec.rows[ec.cy].data.len();
        }
        ec.cx -= 1;
    }

    // Find whitespace after finding this letter (or get to the front of line?)
    while !SEPARATORS.contains(&ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap()){
        if ec.cx == 0 && ec.cy == 0 {return}
        if ec.cx == 0 {return}
        while ec.cx == 0 {
            ec.cy -= 1;
            ec.cx = ec.rows[ec.cy].data.len();
        }
        ec.cx -= 1;
    }

    // Move to letter after whitespace
    ec.cx += 1;
}

fn dd_motion(ec: &mut EditorConfig){
    if ec.numrows == 1 {
        ec.rows[0].data = String::new();
        ec.cx = 0;
        ec.cy = 0;
    } else {
        ec.rows.remove(ec.cy);
        ec.numrows -= 1;
        if ec.cy == ec.numrows {ec.cy -= 1}
    }
    // set all rows after as dirty
    ec.dirty_rows.extend((ec.cy - ec.rowoff)..ec.screenrows);
}

fn ui_motion(ec: &mut EditorConfig){
    ec.cx = ec.rows[ec.cy].indent * TAB_LENGTH;
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn ug_motion(ec: &mut EditorConfig){
    ec.cy = ec.numrows - 1;
}

fn h_motion(ec: &mut EditorConfig){
    if ec.cx > 0 {ec.cx -= 1;}
}

fn i_motion(ec: &mut EditorConfig){
    let _ = stdout().execute(cursor::SetCursorStyle::SteadyBar);
    ec.mode = Mode::Insert;
}

fn j_motion(ec: &mut EditorConfig){
    if ec.cy < ec.numrows - 1 {ec.cy += 1;}
}

fn k_motion(ec: &mut EditorConfig){
    if ec.cy > 0 {ec.cy -= 1;}
}

fn l_motion(ec: &mut EditorConfig){
    if ec.cx < ec.rows[ec.cy].data.len() {ec.cx += 1;}
}

fn v_motion(ec: &mut EditorConfig){
    ec.mode = Mode::Visual
}

fn empty_up(ec: &mut EditorConfig) {
    if ec.cy == 0 {return}
    ec.cy -= 1;
    while !ec.rows[ec.cy].data.is_empty() {
        if ec.cy == 0 {return}
        ec.cy -= 1;
    }
}

fn empty_down(ec: &mut EditorConfig) {
    if ec.cy == ec.numrows - 1 {return}
    ec.cy += 1;
    while !ec.rows[ec.cy].data.is_empty() {
        if ec.cy == ec.numrows - 1 {return}
        ec.cy += 1;
    }
}

fn gg_motion(ec: &mut EditorConfig){
    ec.cy = 0;
}

fn x_motion(ec: &mut EditorConfig){
    if ec.cx == ec.rows[ec.cy].data.len() {return}
    ec.rows[ec.cy].data.remove(ec.cx);
}

fn e_motion(ec: &mut EditorConfig){
    // Move forward 2 (make sure we dont go past eof)
    ec.cx += 2;
    if ec.cy == ec.numrows - 1 && ec.cx >= ec.rows[ec.cy].data.len() {return}

    if ec.cx >= ec.rows[ec.cy].data.len() {
        ec.cy += 1;
        ec.cx = 0;
        if ec.rows[ec.cy].data.is_empty() {return}
        while ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap() == ' ' {ec.cx += 1}
        return
    }

    // Find a separator
    while !SEPARATORS.contains(&ec.rows[ec.cy].data.chars().nth(ec.cx).unwrap()){
        if ec.cy == ec.numrows && ec.cx >= ec.rows[ec.cy].data.len() {return}
        if ec.cx == ec.rows[ec.cy].data.len() - 1 {return}
        ec.cx += 1;
    }

    // go back to the last token
    ec.cx -= 1;
}

/*** Keyboard Event Handling ***/
fn handle_normal(ec: &mut EditorConfig) -> io::Result<bool>  {
let mut motion_done = false;
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            // mark current row dirty (if we leave this row we rand to make lineno dark!)
            ec.dirty_rows.push(ec.cy - ec.rowoff);
            if let KeyCode::Char(c) = key.code {
                motion_done = true;
                // Check for number
                if matches!(key.code, KeyCode::Char(c) if c.is_ascii_digit()){
                    if !ec.motion.is_empty() {
                        ec.motion = String::new();
                    }
                    let num = c.to_digit(10).map(|n| n as u16).unwrap_or(0);
                    ec.motion_count *= 10;
                    ec.motion_count += num as usize;
                    set_status_message(ec, ec.motion_count.to_string())?;
                    return Ok(true);
                }
                ec.motion.push(c);
                let motion = match ec.motion.as_str() {
                    "dd" => dd_motion,
                    "a" => a_motion,
                    "A" => ua_motion,
                    "b" => b_motion,
                    "e" => e_motion,
                    "G" => ug_motion,
                    "gg" => gg_motion,
                    "h" => h_motion,
                    "i" => i_motion,
                    "I" => ui_motion,
                    "j" => j_motion,
                    "k" => k_motion,
                    "l" => l_motion,
                    "o" => o_motion,
                    "O" => uo_motion,
                    "v" => v_motion,
                    "w" => w_motion,
                    "x" => x_motion,
                    ":" => colon,
                    "{" => empty_up,
                    "}" => empty_down,
                    _ => {
                        if ec.motion.len() > 3 {ec.motion = String::default()};
                        let _ = set_status_message(ec, ec.motion.clone());
                        return Ok(false)
                    },
                };
                if ec.motion_count == 0 {
                    motion(ec)
                }
                for _i in 0..ec.motion_count {
                    motion(ec);
                }
                ec.motion_count = 0;
            }
        }
    }
    if motion_done {ec.motion = String::default()};
    Ok(motion_done)
}

fn auto_indent(ec: &mut EditorConfig) {
    let cy = ec.cy;
    let current_line = ec.rows[cy].data.clone();
    let indents = ec.rows[cy].indent;
    let (split_left, split_right) = current_line.split_at(ec.cx);
    
    let leading_spaces = " ".repeat(TAB_LENGTH * ec.rows[cy].indent);

    // Simplified line splitting and insertion
    ec.rows.remove(ec.cy);
    ec.numrows -= 1;
    insert_row(ec, ec.cy, split_left.to_string());
    insert_row(ec, ec.cy + 1, format!("{}{}", leading_spaces, split_right));

    // set indent levels
    ec.rows[cy].indent = indents;
    ec.rows[cy + 1].indent = indents;

    // Calculate indentation for cursor positioning
    let additional_indent = if !split_right.is_empty() && [']', '}', ')'].contains(&split_right.chars().next().unwrap()) {
        // Handle specific closing characters with additional indentation
        let extra_indent_str = " ".repeat(TAB_LENGTH * (indents + 1)); // Adjust the number of spaces as needed
        let _ = set_status_message(ec, format!("|{}|", extra_indent_str));
        insert_row(ec, ec.cy + 1, extra_indent_str.clone());
        ec.rows[cy + 1].indent = indents + 1;
        TAB_LENGTH // Adjust according to your indentation strategy
    } else {
        0
    };
    // set current row to dirty bc we will set cy to next row
    ec.dirty_rows.push(ec.cy - ec.rowoff);

    // set all rows below current as dirty because they will shift
    ec.dirty_rows.extend((ec.cy - ec.rowoff + 2)..ec.screenrows);

    ec.cx = leading_spaces.len() + additional_indent;
    ec.cy += 1;
}

fn handle_insert(ec: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char(c) = key.code {
                let cy: usize = ec.cy;
                if ec.j_flag && c == 'k' {
                    ec.rows[cy].data.remove(ec.cx - 1);
                    ec.cx -= 1;
                    ec.j_flag = false;
                    stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                    ec.mode = Mode::Normal;
                } else {
                    ec.rows[cy].data.insert(ec.cx, c);
                    ec.cx += 1;
                    if c == '{' {
                        ec.rows[cy].data.insert(ec.cx, '}');
                    }else if c == '(' {
                        ec.rows[cy].data.insert(ec.cx, ')');
                    }else if c == '[' {
                        ec.rows[cy].data.insert(ec.cx, ']');
                    } else if ec.cx < ec.rows[cy].data.len() && ((c == '}' && ec.rows[cy].data.chars().nth(ec.cx).unwrap() == '}') ||
                    (c == ')' && ec.rows[cy].data.chars().nth(ec.cx).unwrap() == ')') ||
                    (c == ']' && ec.rows[cy].data.chars().nth(ec.cx).unwrap() == ']')) {
                        ec.rows[cy].data.remove(ec.cx);
                    } else if c == 'j' {
                        ec.j_flag = true;
                    }
                }
            } else if key.code == KeyCode::Left{
                h_motion(ec);
            } else if key.code == KeyCode::Right {
                l_motion(ec);
            } else if key.code == KeyCode::Down {
                j_motion(ec);
            } else if key.code == KeyCode::Up {
                k_motion(ec);
            } else if key.code == KeyCode::Tab {
                let tab_str = " ".repeat(TAB_LENGTH);
                let cy: usize = ec.cy;
                if ec.cx == ec.rows[cy].indent * TAB_LENGTH {
                    ec.rows[cy].indent += 1;
                }
                ec.rows[cy].data.insert_str(ec.cx, &tab_str);
                ec.cx += TAB_LENGTH;
            } else if key.code == KeyCode::Esc {
                if ec.cx > 0 {ec.cx -= 1;}
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                ec.mode = Mode::Normal;
            } else if key.code == KeyCode::Enter {
                auto_indent(ec);
            } else if key.code == KeyCode::Backspace {
                let cy: usize = ec.cy;
                let len = ec.rows[cy].data.len();
                if ec.cx <= len && ec.cx > 0{
                    // If we are changing the indent level, denote this
                    if ec.cx == ec.rows[cy].indent * TAB_LENGTH{
                        ec.rows[cy].indent -= 1;
                    }

                    // Remove char from data
                    ec.rows[cy].data.remove(ec.cx - 1);
                    ec.cx -= 1;
                } else if ec.cx == 0 && ec.cy > 0 {
                    // delete the current line
                    let cur_str = ec.rows[cy].data.clone();
                    let new_cx = ec.rows[cy - 1].data.len();
                    ec.rows[cy - 1].data.push_str(&cur_str);
                    ec.rows.remove(cy);

                    //set all rows below as dirty because they need to shift up
                    ec.dirty_rows.extend((ec.cy - ec.rowoff)..ec.screenrows);
                    ec.cy -= 1;
                    ec.cx = new_cx;
                    ec.numrows -= 1;
                }
            }
            ec.dirty = true;
            return Ok(true)
        }
    }
    Ok(false)
}

fn handle_visual(ec: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                ec.mode = Mode::Normal;
            }
            ec.dirty = true;
            return Ok(true)
        }
    }
    Ok(false)
}

fn handle_command(ec: &mut EditorConfig) -> io::Result<bool>{ 
    if event::poll(std::time::Duration::from_millis(1))?{
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                ec.command = String::default();
                stdout().execute(RestorePosition)?;
                ec.mode = Mode::Normal;
            }
            if let KeyCode::Char(c) = key.code {
                ec.command.push(c);
            }
            if key.code == KeyCode::Backspace {
                ec.command.pop();
            }
            if key.code == KeyCode::Enter {
                match ec.command.as_str() {
                    "w" => {
                        editor_save(ec)?;
                        set_status_message(ec, format!("{} {}L written", ec.filename, ec.numrows))?;
                        stdout().execute(RestorePosition)?;
                    }
                    "q" => {
                        if !(ec.dirty) {
                            disable_raw_mode()?;
                            stdout().execute(LeaveAlternateScreen)?;
                            exit(0);
                        } else {
                            set_status_message(ec, String::from("FILE HAS NOT BEEN SAVED!"))?;
                            stdout().execute(RestorePosition)?;
                        }
                    }
                    "wq" => {
                        editor_save(ec)?;
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
                ec.command = String::default();
                ec.mode = Mode::Normal;
            }
            return Ok(true)
        }
    }
    Ok(false)
}

