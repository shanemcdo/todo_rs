use colored::*;
use structopt::StructOpt;
use std::io::{
    self,
    BufRead,
    prelude::*,
};
use termion::{
    self,
    input::TermRead,
    raw::IntoRawMode,
    event::Key,
};

const TODO_LIST: &str = std::env!("TODO_LIST");
const DONE_LIST: &str = std::env!("TODO_DONE_LIST");
const COLORS_LEN: usize = 12;
const COLORS: [Color; COLORS_LEN] = [
    Color::TrueColor{r: 255, g: 0, b: 0},
    Color::TrueColor{r: 255, g: 128, b: 0},
    Color::TrueColor{r: 255, g: 255, b: 0},
    Color::TrueColor{r: 128, g: 255, b: 0},
    Color::TrueColor{r: 0, g: 255, b: 0},
    Color::TrueColor{r: 0, g: 255, b: 128},
    Color::TrueColor{r: 0, g: 255, b: 255},
    Color::TrueColor{r: 0, g: 128, b: 255},
    Color::TrueColor{r: 0, g: 0, b: 255},
    Color::TrueColor{r: 128, g: 0, b: 255},
    Color::TrueColor{r: 255, g: 0, b: 255},
    Color::TrueColor{r: 255, g: 0, b: 128}
];

#[derive(Copy, Clone, Debug)]
enum ListType {
    Todo,
    Done,
}

impl ListType {
    fn next(&mut self) -> Self {
        match self {
            ListType::Todo => ListType::Done,
            ListType::Done => ListType::Todo,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum InputMode {
    Normal,
    Insert,
}

fn word_wrap(s: String, max_length: usize) -> Vec<String> {
    let mut res = vec![];
    let mut s = s.trim().to_string();
    'outer: loop {
        let mut prev_word = 0;
        for (i, ch) in s.chars().enumerate() {
            if ch == ' ' {
                prev_word = i;
            } else if i + 1 >= max_length {
                res.push(s[..prev_word].to_string());
                s = s[prev_word..]
                    .trim()
                    .to_string();
                continue 'outer;
            }
        }
        res.push(s.clone());
        break;
    }
    res
}

fn save_list(filename: &str, list: &Vec<String>) {
    let mut file = std::fs::File::create(filename).expect("Could not create file");
    for line in list {
        write!(file, "{}\n", line).expect("Could not write to file");
    }
}

fn load_list(filename: &str) -> Vec<String> {
    if let Ok(file) = std::fs::File::open(filename) {
        io::BufReader::new(file)
            .lines()
            .map(|x| x.expect("Could not get line"))
            .collect()
    } else {
        vec![]
    }
}

fn colorize(index: usize) -> Color {
    COLORS[index % COLORS_LEN].clone()
}

/// A program that acts as a todo list
#[derive(Debug, StructOpt)]
#[structopt(name = "todo")]
struct Args {
    /// Directly add an item to the todo list
    #[structopt(short, long, default_value = "")]
    add: String
}

struct ListApp {
    running: bool,
    stdin: termion::input::Keys<termion::AsyncReader>,
    stdout: termion::raw::RawTerminal<io::Stdout>,
    todo: Vec<String>,
    done: Vec<String>,
    current_index: u16,
    list_type: ListType,
    input_mode: InputMode,
    input_string: String,
    terminal_size: (u16, u16),
}

impl ListApp{
    fn new() -> Self{
        Self {
            running: true,
            stdin: termion::async_stdin().keys(),
            stdout: io::stdout().into_raw_mode().unwrap(),
            todo: load_list(TODO_LIST),
            done: load_list(DONE_LIST),
            current_index: 0,
            list_type: ListType::Todo,
            input_mode: InputMode::Normal,
            input_string: "".to_string(),
            terminal_size: termion::terminal_size().expect("Could not get terminal size"),
        }
    }

    fn go_to_current_index(&mut self){
        let list = match self.list_type {
            ListType::Todo => &mut self.todo,
            ListType::Done => &mut self.done,
        };
        // the logic is the position of the current index is the sum
        // is the sum of all the lines before the current line plus 1
        let mut pos = 1;
        for i in 0..self.current_index {
            pos += word_wrap(list[i as usize].clone(), self.terminal_size.0 as usize / 2).len();
        }
        println!("{}", pos);
        let x = match self.list_type {
            ListType::Todo => 1,
            ListType::Done => self.terminal_size.0 / 2,
        };
        write!(self.stdout, "{}", termion::cursor::Goto(x, pos as u16))
            .expect("Could not move cursor");
    }

    fn redraw(&mut self){
        self.clear();
        match self.input_mode {
            InputMode::Normal => {
                self.draw_todo();
                self.draw_done();
                self.go_to_current_index();
            }
            InputMode::Insert => {
                write!(self.stdout, "{} {}", "New item:".blue().bold(), self.input_string)
                    .expect("Could not print message");
            }
        }
        self.stdout.flush().expect("Could not flush");
    }

    fn run(&mut self) {
        self.redraw();
        while self.running {
            self.terminal_size = termion::terminal_size().expect("Could not get terminal size");
            if self.kbin() {
                self.redraw();
            }
        }
        save_list(TODO_LIST, &self.todo);
        save_list(DONE_LIST, &self.done);
        self.clear();
    }

    fn draw_todo(&mut self){
        let max = self.terminal_size.0 / 2 - 4;
        let mut idx = 0u16;
        for line in &self.todo {
            if line.len() < max as usize {
                write!(
                    self.stdout,
                    "{}[ ] {}",
                    termion::cursor::Goto(1, idx + 1),
                    line.color(colorize(idx as usize)),
                ).expect("Could not write line");
                idx += 1;
            } else {
                let mut first = true;
                for subline in word_wrap(line.clone(), max as usize) {
                    if first {
                        first = false;
                        write!(
                            self.stdout,
                            "{}[ ] {}",
                            termion::cursor::Goto(1, idx + 1),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    } else {
                        write!(
                            self.stdout,
                            "{}    {}",
                            termion::cursor::Goto(1, idx + 1),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    }
                    idx += 1;
                }
            }
        }
    }

    fn draw_done(&mut self){
        let x = self.terminal_size.0 / 2;
        let max = self.terminal_size.0 / 2 - 4;
        let mut idx = 0u16;
        for line in &self.done {
            if line.len() < max as usize {
                write!(
                    self.stdout,
                    "{}[{}] {}",
                    termion::cursor::Goto(x, idx + 1),
                    "X".red().bold(),
                    line.color(colorize(idx as usize)),
                ).expect("Could not write line");
                idx += 1;
            } else {
                let mut first = true;
                for subline in word_wrap(line.clone(), max as usize) {
                    if first {
                        first = false;
                        write!(
                            self.stdout,
                            "{}[ ] {}",
                            termion::cursor::Goto(x, idx + 1),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    } else {
                        write!(
                            self.stdout,
                            "{}    {}",
                            termion::cursor::Goto(x, idx + 1),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    }
                    idx += 1;
                }
            }
        }
    }

    fn clear(&mut self) {
        write!(
            self.stdout,
            "{}{}",
            termion::clear::All,
            termion::cursor::Goto(1, 1),
        )
            .expect("Could not clear screen");
    }

    fn move_up(&mut self){
        let len = self.get_curr_list_len();
        if len == 0 {
            return
        }
        if self.current_index == 0 {
            self.current_index = len;
        }
        self.current_index -= 1;
    }

    fn move_down(&mut self){
        let len = self.get_curr_list_len();
        if len == 0 {
            return
        }
        self.current_index += 1;
        self.current_index %= len;
    }

    fn get_curr_list_len(&self) -> u16{
        match self.list_type {
            ListType::Todo => self.todo.len() as u16,
            ListType::Done => self.done.len() as u16,
        }
    }

    fn swap_list(&mut self){
        self.list_type = self.list_type.next();
        let len = self.get_curr_list_len();
        if len == 0 {
            self.current_index = 0;
            return
        }
        self.current_index %= len;
    }
    
    fn check_item(&mut self){
        let mut len = self.todo.len() as u16;
        if len == 0 {
            return
        }
        self.done.push(self.todo.remove(self.current_index as usize));
        len -= 1;
        if len == 0 {
            self.current_index = 0;
            return
        }
        if self.current_index >= len {
            self.current_index = len - 1;
        }
    }

    fn uncheck_item(&mut self){
        let mut len = self.done.len() as u16;
        if len == 0 {
            return
        }
        self.todo.push(self.done.remove(self.current_index as usize));
        len -= 1;
        if len == 0 {
            self.current_index = 0;
            return
        }
        if self.current_index >= len {
            self.current_index = len - 1;
        }
    }

    fn delete_item(&mut self){
        let _ = self.done.remove(self.current_index as usize);
        let len = self.done.len() as u16;
        if len == 0 {
            self.current_index = 0;
            return
        }
        if self.current_index >= len {
            self.current_index = len - 1;
        }
    }

    fn shift_up(&mut self){
        let list = match self.list_type {
            ListType::Todo => &mut self.todo,
            ListType::Done => &mut self.done,
        };
        let len = list.len();
        if len <= 0 {
            return;
        }
        let idx = self.current_index as usize;
        if idx > 0 {
            list.swap(idx, idx - 1);
        } else {
            let item = list.remove(0);
            list.push(item);
        }
        self.move_up();
    }

    fn shift_down(&mut self){
        let list = match self.list_type {
            ListType::Todo => &mut self.todo,
            ListType::Done => &mut self.done,
        };
        let len = list.len();
        if len <= 0 {
            return;
        }
        let idx = self.current_index as usize;
        if idx + 1 < len {
            list.swap(idx, idx + 1);
        } else {
            let item = list.remove(len - 1);
            list.insert(0, item);
        }
        self.move_down();
    }

    /// handle keyboard input
    /// returns true if redraw needs to be called again, otherwise returns false
    fn kbin(&mut self) -> bool {
        if let Some(Ok(key)) = self.stdin.next() {
            match self.input_mode {
                InputMode::Normal => match (key, self.list_type) {
                    (Key::Char('q') | Key::Esc, _) => self.running = false,
                    (Key::Char('d') | Key::Char('x') | Key::Insert, ListType::Todo) => self.check_item(),
                    (Key::Char('x') | Key::Char('\n'), ListType::Done) => self.uncheck_item(),
                    (Key::Char('d') | Key::Backspace, ListType::Done) => self.delete_item(),
                    (Key::Char(ch), _) => match ch {
                        'a' | 'i' => self.input_mode = InputMode::Insert,
                        'h' | 'l' => self.swap_list(),
                        'j' => self.move_down(),
                        'J' => self.shift_down(),
                        'k' => self.move_up(),
                        'K' => self.shift_up(),
                        _ => return false,
                    }
                    _ => return false,
                }
                InputMode::Insert => match key {
                    Key::Esc => {
                        self.input_mode = InputMode::Normal;
                        self.input_string = "".to_string();
                    }
                    Key::Backspace => {self.input_string.pop().unwrap_or('\0');},
                    Key::Char('\n') => {
                        self.input_mode = InputMode::Normal;
                        let mut s = "".to_string();
                        std::mem::swap(&mut s, &mut self.input_string);
                        self.todo.push(s);
                    },
                    Key::Char(ch) => self.input_string.push(ch),
                    _ => return false,
                }
            }
        } else {
            return false;
        }
        true
    }
}

fn main() {
    let args = Args::from_args();
    if args.add != "" {
        let mut list = load_list(TODO_LIST);
        list.push(args.add);
        save_list(TODO_LIST, &list);
    } else {
        ListApp::new().run();
    }
}
