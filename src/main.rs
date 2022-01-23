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

macro_rules! get_list {
    ($app: expr, $list_type: expr) => (
        match $list_type {
            ListType::Todo => &$app.todo,
            ListType::Done => &$app.done,
        }
    );

    ($app: expr, mut $list_type: expr) => (
        match $list_type {
            ListType::Todo => &mut $app.todo,
            ListType::Done => &mut $app.done,
        }
    );
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum ListType {
    Todo,
    Done,
}

impl ListType {
    fn next(&self) -> Self {
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

fn print_list(list: &Vec<String>) {
    for line in list {
        println!("{}", line);
    }
}

fn print_todo(){
    print_list(&load_list(TODO_LIST));
}

fn word_wrap(s: &str, max_length: usize) -> Vec<String> {
    let mut res = vec![];
    let mut s = s.trim().to_string();
    'outer: loop {
        let mut prev_word = 0;
        for (i, ch) in s.chars().enumerate() {
            if !ch.is_alphanumeric(){
                prev_word = i;
            }
            if i + 1 >= max_length {
                if prev_word == 0 {
                    prev_word = i; // if one word is too long u can wrap it
                }
                res.push(s[..prev_word].to_string()); // append to result
                s = s[prev_word..] // remove part pushed to result
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

fn color(index: usize) -> Color {
    COLORS[index % COLORS_LEN].clone()
}

struct List {
    items: Vec<String>,
    list_type: ListType,
    current_index: usize,
    y_offset: usize,
}

impl List {
    fn new(items: Vec<String>, list_type: ListType) -> Self {
        Self {
            items,
            list_type,
            current_index: 0,
            y_offset: 0,
        }
    }

    fn draw(&mut self, pos: (u16, u16), size: (u16, u16)){
        if self.out_of_bounds(size){
            self.update_y_offset(size);
        }
        let checkbox = self.get_checkbox();
        let mut offset = self.y_offset as u16;
        print!(
            "{}[{}]",
            termion::cursor::Goto(pos.0, pos.1),
            self.get_title(),
        );
        let max = size.0 - CHECKBOX_WIDTH as u16;
        let mut idx = 0u16;
        'outer: for line in &self.items {
            let mut first = true;
            for subline in word_wrap(&line, max as usize) {
                let checkbox = if first {
                    first = false;
                    &checkbox
                } else {
                    "    "
                };
                if idx + 2 > size.1 { // offscreen
                    break 'outer;
                }
                if idx < offset {
                    offset -= 1;
                    continue;
                }
                print!(
                    "{}{}{}",
                    termion::cursor::Goto(pos.0, pos.1 + idx + 1),
                    checkbox,
                    subline.color(color(idx as usize)),
                );
                idx += 1;
            }
        }
    }

    fn get_title(&self) -> ColoredString {
        match self.list_type {
            ListType::Todo => "Todo".green().bold(),
            ListType::Done => "Done".red().bold(),
        }
    }

    fn get_checkbox(&self) -> String {
        match self.list_type {
            ListType::Todo => "[ ] ".to_string(),
            ListType::Done => format!("[{}] ", "X".red().bold()),
        }
    }

    fn move_to_top(&mut self){
        self.current_index = 0;
    }

    fn move_to_bottom(&mut self){
        let len = self.items.len();
        self.current_index = if len == 0 {
            0
        } else {
            len - 1
        };
    }

    fn move_up(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        if self.current_index == 0 {
            self.current_index = len - 1;
        } else {
            self.current_index -= 1;
        }
    }

    fn move_down(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.current_index += 1;
        self.current_index %= len;
    }

    fn shift_up(&mut self) {
        let len = self.items.len();
        if len <= 0 {
            return;
        }
        if self.current_index > 0 {
            self.items.swap(self.current_index, self.current_index - 1);
        } else {
            let item = self.items.remove(0);
            self.items.push(item);
        }
        self.move_up();
    }

    fn shift_down(&mut self) {
        let len = self.items.len();
        if len <= 0 {
            return;
        }
        if self.current_index + 1 < len {
            self.items.swap(self.current_index, self.current_index + 1);
        } else {
            let item = self.items.remove(len - 1);
            self.items.insert(0, item)
        }
        self.move_down();
    }

    fn remove(&mut self) -> Option<String> {
        if self.items.len() < 1 {
            None
        } else {
            let res = Some(self.items.remove(self.current_index));
            let len = self.items.len();
            if len == 0 {
                self.current_index = 0
            } else if self.current_index >= len {
                self.current_index = len - 1;
            }
            res
        }
    }

    fn add(&mut self, item: String) {
        self.items.push(item);
    }
    
    fn insert(&mut self, item: String, index: usize) {
        self.items.insert(index, item); 
    }

    fn insert_before(&mut self, item: String) {
        self.insert(item, self.current_index);
    }

    fn insert_after(&mut self, item: String) {
        self.insert(item, self.current_index + 1);
    }

    fn set_current(&mut self, item: String) {
        self.items[self.current_index] = item;
    }

    fn take_current(&mut self) -> Option<String> {
        if self.items.len() == 0 {
            None
        } else {
            Some(std::mem::take(&mut self.items[self.current_index]))
        }
    }

    fn get_y_pos(&self, size: (u16, u16)) -> usize {
        let max = size.0 as usize - CHECKBOX_WIDTH;
        let mut y = 1; // start at one for title
        // the logic is the position of the current index is the sum
        // is the sum of all the lines before the current line
        // plus 1 for the title offset
        for i in 0..self.current_index {
            y += word_wrap(&self.items[i], max).len();
        }
        y
    }
    
    fn go_to_current_index(&self, pos: (u16, u16), size: (u16, u16)) {
        let y = self.get_y_pos(size).checked_sub(self.y_offset).unwrap_or(1) as u16;
        print!(
            "{}",
            termion::cursor::Goto(pos.0, pos.1 + y),
        );
    }

    fn update_y_offset(&mut self, size: (u16, u16)) {
        let y = self.get_y_pos(size);
        self.y_offset = if y  > self.y_offset {
            (y + 1).checked_sub(size.1 as usize).unwrap_or(0)
        } else {
            y - 1
        }
    }

    fn out_of_bounds(&self, size: (u16, u16)) -> bool {
        let y = self.get_y_pos(size);
        if y + 1 > size.1 as usize + self.y_offset || y <= self.y_offset {
            true
        } else {
            false
        }
    }
}

struct TodoApp {
    running: bool,
    stdin: termion::input::Keys<termion::AsyncReader>,
    stdout: termion::raw::RawTerminal<io::Stdout>,
    todo: List,
    done: List,
    list_type: ListType,
    input_mode: InputMode,
    input_string: String,
    input_string_index: usize,
    terminal_size: (u16, u16),
    one_pane: bool,
}

impl TodoApp{
    fn new() -> Self{
        let terminal_size = termion::terminal_size().expect("Could not get terminal size"); 
        Self {
            running: true,
            stdin: termion::async_stdin().keys(),
            stdout: io::stdout().into_raw_mode().unwrap(),
            todo: List::new(load_list(TODO_LIST), ListType::Todo),
            done: List::new(load_list(DONE_LIST), ListType::Done),
            list_type: ListType::Todo,
            input_mode: InputMode::Normal,
            input_string: "".to_string(),
            input_string_index: 0,
            terminal_size,
            one_pane: terminal_size.0 <= MAX_WIDTH_SINGLE_PANE,
        }
    }

    fn go_to_current_index(&self){
        let size = if self.one_pane {
            self.terminal_size
        } else {
            (self.terminal_size.0 / 2, self.terminal_size.1)
        };
        match self.list_type {
            ListType::Todo => self.todo.go_to_current_index(
                (1, 1),
                size,
            ),
            ListType::Done => self.done.go_to_current_index(
                if self.one_pane {
                    (1, 1)
                } else {
                    (self.terminal_size.0 / 2, 1)
                },
                size,
            ),
        }
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
            let size = termion::terminal_size().expect("Could not get terminal size");
            if self.terminal_size != size {
                self.terminal_size = size;
                self.one_pane = self.terminal_size.0 <= MAX_WIDTH_SINGLE_PANE;
                self.redraw();
            }
            if self.kbin() {
                self.redraw();
            }
        }
        save_list(TODO_LIST, &self.todo.items);
        save_list(DONE_LIST, &self.done.items);
        self.clear();
    }

    fn draw_todo(&mut self){
        self.todo.draw(
            (1, 1),
            if self.one_pane {
                self.terminal_size
            } else {
                (self.terminal_size.0 / 2, self.terminal_size.1)
            }
        );
    }

    fn draw_done(&mut self){
        self.done.draw(
            if self.one_pane {
                (1, 1)
            } else {
                (self.terminal_size.0 / 2, 1)
            },
            if self.one_pane {
                self.terminal_size
            } else {
                (self.terminal_size.0 / 2, self.terminal_size.1)
            }
        );
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

    fn swap_list(&mut self){
        self.list_type = self.list_type.next();
    }

    fn check_item(&mut self){
        if let Some(item) = self.todo.remove() {
            self.done.add(item);
        }
    }

    fn uncheck_item(&mut self){
        if let Some(item) = self.done.remove() {
            self.todo.add(item);
        }
    }

    fn delete_item(&mut self){
        self.done.remove();
    }

    /// handle keyboard input
    /// returns true if redraw needs to be called again, otherwise returns false
    fn kbin(&mut self) -> bool {
        if let Some(Ok(key)) = self.stdin.next() {
            let list = get_list!(self, mut self.list_type);
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
                            if let Some(item) = list.take_current() {
                                self.input_string_index = item.len();
                                self.input_string = item;
                            }
                        },
                        'a' | 'i' => self.input_mode = InputMode::Insert(InputDestination::NewItem),
                        'h' | 'l' => self.swap_list(),
                        'j' => list.move_down(),
                        'J' => list.shift_down(),
                        'k' => list.move_up(),
                        'K' => list.shift_up(),
                        'g' => list.move_to_top(),
                        'G' => list.move_to_bottom(),
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
                        let s = std::mem::take(&mut self.input_string);
                        match dest {
                            InputDestination::NewItem => self.todo.add(s),
                            InputDestination::NewItemBefore => {
                                self.todo.insert_before(s);
                            }
                            InputDestination::NewItemAfter => {
                                self.todo.insert_after(s);
                            }
                            InputDestination::EditItem => list.set_current(s),
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
    add: String,
    /// Print list instead of interactive prompt
    #[structopt(short, long)]
    print: bool,
}

fn main() {
    let args = Args::from_args();
    termion::terminal_size().unwrap_or_else(|_|{
        print_todo();
        std::process::exit(0)
    });
    if args.add != "" {
        let mut list = load_list(TODO_LIST);
        list.push(args.add);
        save_list(TODO_LIST, &list);
    } else if args.print {
        print_todo();
    } else {
        TodoApp::new().run();
    }
}
