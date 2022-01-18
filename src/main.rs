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

fn colorize_list(list: &Vec<String>) -> Vec<ColoredString> {
    list.into_iter()
        .enumerate()
        .map(|(index, string)| string.color(colorize(index)))
        .collect()
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
        }
    }

    fn redraw(&mut self){
        self.clear();
        match self.list_type {
            ListType::Todo => self.draw_todo(),
            ListType::Done => self.draw_done(),
        }
        write!(self.stdout, "{}", termion::cursor::Goto(1, self.current_index + 1))
               .expect("Could not move cursor");
        self.stdout.flush().expect("Could not flush");
    }

    fn run(&mut self) {
        self.redraw();
        while self.running {
            if self.kbin() {
                self.redraw();
            }
        }
        save_list(TODO_LIST, &self.todo);
        save_list(DONE_LIST, &self.done);
        self.clear();
    }

    fn draw_todo(&mut self){
        let colorized = colorize_list(&self.todo);
        for (i, line) in colorized.into_iter().enumerate() {
            write!(
                self.stdout,
                "{}[ ] {}",
                termion::cursor::Goto(1, i as u16 + 1),
                line,
            )
                .expect("Could not write line");
        }
    }

    fn draw_done(&mut self){
        let colorized = colorize_list(&self.done);
        for (i, line) in colorized.into_iter().enumerate() {
            write!(
                self.stdout,
                "{}[{}] {}",
                termion::cursor::Goto(1, i as u16 + 1),
                "X".bright_red(),
                line,
            )
                .expect("Could not write line");
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
    
    fn kbin(&mut self) -> bool {
        if let Some(Ok(key)) = self.stdin.next() {
            match self.input_mode {
                InputMode::Normal => match (key, self.list_type) {
                    (Key::Char('q') | Key::Esc, _) => self.running = false,
                    (Key::Char('d') | Key::Char('x') | Key::Insert, ListType::Todo) => self.check_item(),
                    (Key::Char('x') | Key::Insert, ListType::Done) => self.uncheck_item(),
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
                    Key::Esc => self.input_mode = InputMode::Normal,
                    Key::Backspace => (),
                    Key::Insert => (),
                    Key::Char(ch) => (),
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
