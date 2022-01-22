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

fn colorize(index: usize) -> Color {
    COLORS[index % COLORS_LEN].clone()
}

struct List {
    items: Vec<String>,
    list_type: ListType,
    current_index: usize,
    prev_y_offset: usize,
    y_offset: usize,
}

impl List {
    fn new(items: Vec<String>, list_type: ListType) -> Self {
        Self {
            items,
            list_type,
            current_index: 0,
            prev_y_offset: 0,
            y_offset: 0,
        }
    }

    fn draw(&mut self, pos: (u16, u16), bounds: (u16, u16)){
        let title = self.get_title();
        let checkbox = self.get_checkbox();
        let mut offset = self.y_offset as u16;
        print!(
            "{}[{}]",
            termion::cursor::Goto(pos.0, pos.1),
            title,
        );
        let max = bounds.0 - CHECKBOX_WIDTH as u16;
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
                if idx + 2 > bounds.1 { // offscreen
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
                    subline.color(colorize(idx as usize)),
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

    fn move_up(&mut self) {
        let len = self.items.len();
        if self.current_index == 0 {
            self.current_index = len - 1;
        } else {
            self.current_index -= 1;
        }
    }

    fn move_down(&mut self) {
        let len = self.items.len();
        self.current_index += 1;
        if self.current_index >= len {
            self.current_index = 0;
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

    fn go_to_current_index(&mut self){
        let max = self.get_max_word_wrap_length();
        let list = get_list!(self, self.list_type);
        let offset = self.get_y_offset(self.list_type);
        // the logic is the position of the current index is the sum
        // is the sum of all the lines before the current line plus 1
        // plus 1 again for the title offset
        let mut pos = 2;
        for i in 0..self.current_index {
            pos += word_wrap(&list[i as usize], max).len();
        }
        let x = self.get_x_pos(self.list_type);
        let pos = (pos as u16).checked_sub(offset).unwrap_or(2);
        write!(self.stdout, "{}", termion::cursor::Goto(x, pos))
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
        save_list(TODO_LIST, &self.todo);
        save_list(DONE_LIST, &self.done);
        self.clear();
    }

    fn draw_todo(&mut self){
        self.draw_list(ListType::Todo);
    }

    fn draw_done(&mut self){
        self.draw_list(ListType::Done);
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
        let len = self.get_list_len(self.list_type);
        if len == 0 {
            return
        }
        if self.current_index == 0 {
            self.current_index = len;
        }
        self.current_index -= 1;
    }

    fn move_down(&mut self){
        let len = self.get_list_len(self.list_type);
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
        self.current_index = self.get_list_len(self.list_type) - 1;
    }

    fn get_y_offset(&self, list_type: ListType) -> u16 {
        let list = get_list!(self, list_type);
        let max = self.get_max_word_wrap_length();
        let mut total = 1u16;
        for (i, line) in list.into_iter().enumerate() {
            if i > self.current_index as usize {
                break;
            }
            let l = word_wrap(&line, max).len() as u16;
            total += l;
        }
        total.checked_sub(self.terminal_size.1).unwrap_or(0)
    }

    fn get_title(&self, list_type: ListType) -> ColoredString {
        match list_type {
            ListType::Todo => "Todo".green().bold(),
            ListType::Done => "Done".red().bold(),
        }
    }

    fn get_checkbox(&self, list_type: ListType) -> String {
        match list_type {
            ListType::Todo => "[ ] ".to_string(),
            ListType::Done => format!("[{}] ", "X".red().bold()),
        }
    }

    fn get_x_pos(&self, list_type: ListType) -> u16 {
        match list_type {
            ListType::Done if !self.one_pane => self.terminal_size.0 / 2,
            _ => 1, // if self.one_pane or ListType::Todo
        }
    }

    fn get_list_len(&self, list_type: ListType) -> u16{
        match list_type {
            ListType::Todo => self.todo.len() as u16,
            ListType::Done => self.done.len() as u16,
        }
    }

    fn get_max_word_wrap_length(&self) -> usize{
        if self.one_pane {
            self.terminal_size.0 as usize - CHECKBOX_WIDTH
        } else {
            self.terminal_size.0 as usize / 2 - CHECKBOX_WIDTH
         }
    }

    fn swap_list(&mut self){
        self.list_type = self.list_type.next();
        let len = self.get_list_len(self.list_type);
        if len == 0 {
            self.current_index = 0;
            return
        } else if self.current_index > len {
            self.current_index = len - 1;
        }
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
                            let list = get_list!(self, self.list_type);
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
                        let s = std::mem::take(&mut self.input_string);
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
                                let list = get_list!(self, mut self.list_type);
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
