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
const MAX_WIDTH_SINGLE_PANE: u16 = 55;
const CHECKBOX_WIDTH: usize = 4;

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
enum InputDestination {
    NewItem,
    NewItemBefore,
    NewItemAfter,
    EditItem,
}

#[derive(Copy, Clone, Debug)]
enum InputMode {
    Normal,
    Insert(InputDestination),
}

fn word_wrap(s: String, max_length: usize) -> Vec<String> {
    let mut res = vec![];
    let mut s = s.trim().to_string();
    'outer: loop {
        let mut prev_word = 0;
        for (i, ch) in s.chars().enumerate() {
            match ch {
                'a'..='z' | 'A'..='Z' => (), // if its a letter
                _ => prev_word = i, // its a sperator
            }
            if i + 1 >= max_length {
                if prev_word == 0 {
                    prev_word = i; // if one word is too long u can wrap it
                }
                res.push(s[..prev_word].to_string()); // append to result
                s = s[prev_word..] // remove part pushed to result
                    .trim()
                    .to_string();
                continue 'outer; // exit outer loop
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
#[structopt(
    name = "todo",
    usage = "todo [options]",
    about = r#"A todo program that tracks a two list of completed and uncompleted items
    environment variables TODO_LIST and TODO_DONE_LIST must be paths to text files to be used
    controls:
        NORMAL MODE:
            q, Esc       ->  Quit
            d, x, Enter  ->  Move item to completed (when hovering todos)
            x, Enter     ->  Move item to todo (when hovering completed)
            d            ->  Delete item from completed
            a, i         ->  Enter insert mode
            h, l         ->  Move from todo to completed
            g            ->  Move to top of list
            G            ->  Move to bottom of list
            j            ->  Move down on a list
            k            ->  Move up on a list
            J            ->  Drag an element down on a list
            K            ->  Drag an element up on a list
        INSERT MODE:
            Esc          ->  Exit insert mode
            Enter        ->  Add writen todo to list
            Backspace    ->  Remove a character from the label
            other keys   ->  write label for todo"#
    )]
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
    input_string_index: usize,
    terminal_size: (u16, u16),
    one_pane: bool,
}

impl ListApp{
    fn new() -> Self{
        let terminal_size = termion::terminal_size().expect("Could not get terminal size"); 
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
            input_string_index: 0,
            terminal_size,
            one_pane: terminal_size.0 <= MAX_WIDTH_SINGLE_PANE,
        }
    }

    fn get_max_word_wrap_length(&self) -> usize{
        if self.one_pane {
            self.terminal_size.0 as usize - CHECKBOX_WIDTH
        } else {
            self.terminal_size.0 as usize / 2 - CHECKBOX_WIDTH
         }
    }

    fn go_to_current_index(&mut self){
        let max = self.get_max_word_wrap_length();
        let list = match self.list_type {
            ListType::Todo => &mut self.todo,
            ListType::Done => &mut self.done,
        };
        // the logic is the position of the current index is the sum
        // is the sum of all the lines before the current line plus 1
        // plus 1 again for the title offset
        let mut pos = 2;
        for i in 0..self.current_index {
            pos += word_wrap(list[i as usize].clone(), max).len();
        }
        let x = match self.list_type {
            ListType::Done if !self.one_pane => self.terminal_size.0 / 2,
            _ => 1, // if self.one_pane or ListType::Todo
        };
        write!(self.stdout, "{}", termion::cursor::Goto(x, pos as u16))
            .expect("Could not move cursor");
    }

    fn redraw(&mut self){
        self.clear();
        match self.input_mode {
            InputMode::Normal => {
                if self.one_pane {
                    match self.list_type {
                        ListType::Todo => self.draw_todo(),
                        ListType::Done => self.draw_done(),
                    }
                } else {
                    self.draw_todo();
                    self.draw_done();
                }
                self.go_to_current_index();
            }
            InputMode::Insert(dest) => {
                let leader = match dest {
                    InputDestination::NewItem => "New item: ".blue().bold(),
                    InputDestination::NewItemBefore => "New item before current: ".magenta().bold(),
                    InputDestination::NewItemAfter => "New item after current: ".purple().bold(),
                    InputDestination::EditItem => "Edit item: ".green().bold(),
                };
                write!(
                    self.stdout,
                    "{}{}{}",
                    leader,
                    self.input_string,
                    termion::cursor::Goto(leader.len() as u16 + self.input_string_index as u16 + 1, 1),
                ).expect("Could not print message");
            }
        }
        self.stdout.flush().expect("Could not flush");
    }

    fn run(&mut self) {
        self.redraw();
        while self.running {
            self.terminal_size = termion::terminal_size().expect("Could not get terminal size");
            self.one_pane = self.terminal_size.0 <= MAX_WIDTH_SINGLE_PANE;
            if self.kbin() {
                self.redraw();
            }
        }
        save_list(TODO_LIST, &self.todo);
        save_list(DONE_LIST, &self.done);
        self.clear();
    }

    fn draw_todo(&mut self){
        write!(
            self.stdout,
            "{}[{}]",
            termion::cursor::Goto(1, 1),
            "Todo".green().bold(),
        ).expect("Could not write to stdout");
        let max = self.get_max_word_wrap_length();
        let mut idx = 0u16;
        for line in &self.todo {
            if line.len() < max as usize {
                write!(
                    self.stdout,
                    "{}[ ] {}",
                    termion::cursor::Goto(1, idx + 2),
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
                            termion::cursor::Goto(1, idx + 2),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    } else {
                        write!(
                            self.stdout,
                            "{}    {}",
                            termion::cursor::Goto(1, idx + 2),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    }
                    idx += 1;
                }
            }
        }
    }

    fn draw_done(&mut self){
        let x = if self.one_pane {
            1
        } else {
            self.terminal_size.0 / 2
        };
        write!(
            self.stdout,
            "{}[{}]",
            termion::cursor::Goto(x, 1),
            "Completed".red().bold(),
        ).expect("Could not write to stdout");
        let max = self.get_max_word_wrap_length();
        let mut idx = 0u16;
        for line in &self.done {
            if line.len() < max as usize {
                write!(
                    self.stdout,
                    "{}[{}] {}",
                    termion::cursor::Goto(x, idx + 2),
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
                            "{}[{}] {}",
                            termion::cursor::Goto(x, idx + 2),
                            "X".red().bold(),
                            subline.color(colorize(idx as usize)),
                        ).expect("Could not write line");
                    } else {
                        write!(
                            self.stdout,
                            "{}    {}",
                            termion::cursor::Goto(x, idx + 2),
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

    fn move_to_top(&mut self){
        self.current_index = 0;
    }

    fn move_to_bottom(&mut self){
        self.current_index = self.get_curr_list_len() - 1;
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
                    (Key::Char('d') | Key::Char('x') | Key::Char('\n'), ListType::Todo) => self.check_item(),
                    (Key::Char('x') | Key::Char('\n'), ListType::Done) => self.uncheck_item(),
                    (Key::Char('d') | Key::Backspace, ListType::Done) => self.delete_item(),
                    (Key::Char('O'), ListType::Todo) => self.input_mode = InputMode::Insert(InputDestination::NewItemBefore),
                    (Key::Char('o'), ListType::Todo) => self.input_mode = InputMode::Insert(InputDestination::NewItemAfter),
                    (Key::Char(ch), _) => match ch {
                        'e' => {
                            self.input_mode = InputMode::Insert(InputDestination::EditItem);
                            let list = match self.list_type {
                                ListType::Todo => &self.todo,
                                ListType::Done => &self.done,
                            };
                            self.input_string = list[self.current_index as usize].clone();
                            self.input_string_index = self.input_string.len();
                        },
                        'a' | 'i' => self.input_mode = InputMode::Insert(InputDestination::NewItem),
                        'h' | 'l' => self.swap_list(),
                        'j' => self.move_down(),
                        'J' => self.shift_down(),
                        'k' => self.move_up(),
                        'K' => self.shift_up(),
                        'g' => self.move_to_top(),
                        'G' => self.move_to_bottom(),
                        _ => return false,
                    }
                    _ => return false,
                }
                InputMode::Insert(dest) => match key {
                    Key::Esc => {
                        if let (Some(Ok(Key::Char(brack))), Some(Ok(Key::Char(letter)))) = (self.stdin.next(), self.stdin.next()) {
                            match (brack, letter) {
                                ('[', 'D') => { // left
                                    if self.input_string_index >= 1 {
                                        self.input_string_index -= 1;
                                    }
                                },
                                ('[', 'C') => { // right
                                    let len = self.input_string.len();
                                    self.input_string_index += 1;
                                    if self.input_string_index > len {
                                        self.input_string_index = len;
                                    }
                                },
                                ('[', _) => (),
                                _ => panic!("Unexpected key sequence"),
                            }
                            return true;
                        }
                        self.input_mode = InputMode::Normal;
                        self.input_string = "".to_string();
                        self.input_string_index = 0;
                    }
                    Key::Backspace => {
                        if self.input_string_index >= 1 {
                            self.input_string_index -= 1;
                        }
                        self.input_string.remove(self.input_string_index);
                    },
                    Key::Char('\n') => {
                        self.input_mode = InputMode::Normal;
                        self.input_string_index = 0;
                        let mut s = "".to_string();
                        std::mem::swap(&mut s, &mut self.input_string);
                        match dest {
                            InputDestination::NewItem => self.todo.push(s),
                            InputDestination::NewItemBefore => {
                                let idx = self.current_index as usize; // appease borrow checker
                                self.todo.insert(idx, s);
                            }
                            InputDestination::NewItemAfter => {
                                let idx = self.current_index as usize + 1; // appease borrow checker
                                self.todo.insert(idx, s);
                            }
                            InputDestination::EditItem => {
                                let list = match self.list_type {
                                    ListType::Todo => &mut self.todo,
                                    ListType::Done => &mut self.done,
                                };
                                list[self.current_index as usize] = s;
                            },
                        }
                    },
                    Key::Char(ch) => {
                        self.input_string.insert(self.input_string_index, ch);
                        self.input_string_index += 1;
                    }
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
