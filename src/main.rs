use std::io::{self, prelude::*, BufRead};
use structopt::StructOpt;
use std::time::Duration;
use crossterm::{
    queue,
    cursor,
    terminal,
    tty::IsTty,
    event::{self, Event, KeyCode},
    style::{
        Color,
        Print,
        Stylize,
        PrintStyledContent,
    },
};

const TODO_LIST: &str = std::env!("TODO_LIST");
const DONE_LIST: &str = std::env!("TODO_DONE_LIST");
const COLORS_LEN: usize = 12;
const COLORS: [Color; COLORS_LEN] = [
    Color::Rgb{r: 255, g: 0,   b: 0},
    Color::Rgb{r: 255, g: 128, b: 0},
    Color::Rgb{r: 255, g: 255, b: 0},
    Color::Rgb{r: 128, g: 255, b: 0},
    Color::Rgb{r: 0,   g: 255, b: 0},
    Color::Rgb{r: 0,   g: 255, b: 128},
    Color::Rgb{r: 0,   g: 255, b: 255},
    Color::Rgb{r: 0,   g: 128, b: 255},
    Color::Rgb{r: 0,   g: 0,   b: 255},
    Color::Rgb{r: 128, g: 0,   b: 255},
    Color::Rgb{r: 255, g: 0,   b: 255},
    Color::Rgb{r: 255, g: 0,   b: 128}
];
const MAX_WIDTH_SINGLE_PANE: u16 = 55;
const CHECKBOX_WIDTH: usize = 4;

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

fn print_list(list: &[String]) {
    for line in list {
        println!("{}", line);
    }
}

fn print_todo() {
    print_list(&load_list(TODO_LIST));
}

fn print_done() {
    print_list(&load_list(DONE_LIST));
}

fn word_wrap(s: &str, max_length: usize) -> Vec<String> {
    let mut res = vec![];
    let mut s = s.trim().to_string();
    'outer: loop {
        for i in 0..s.len() {
            if i + 1 >= max_length {
                let prev_word = if let Some(val) = s[..i].rfind(char::is_whitespace) {
                    val // find end of most recent word
                } else {
                    i // no whitespace; break word
                };
                res.push(s[..prev_word].to_string()); // append to result
                s = s[prev_word..] // remove part pushed to result
                    .trim()
                    .to_string();
                continue 'outer;
            }
        }
        res.push(s);
        break;
    }
    res
}

fn save_list(filename: &str, list: &[String]) {
    let mut file = std::fs::File::create(filename).expect("Could not create file");
    for line in list {
        writeln!(file, "{}", line).expect("Could not write to file");
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
    COLORS[index % COLORS_LEN]
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

    fn draw(&mut self, pos: (u16, u16), size: (u16, u16), stdout: &mut io::Stdout) -> crossterm::Result<()> {
        if self.out_of_bounds(size) {
            self.update_y_offset(size);
        }
        let checkbox = self.get_checkbox();
        let mut offset = self.y_offset as u16;
        queue!(
            stdout,
            cursor::MoveTo(pos.0, pos.1),
            self.get_title(),
        )?;
        let max = self.get_max_line_width(size);
        let mut idx = 0u16;
        'outer: for line in &self.items {
            let mut first = true;
            for subline in word_wrap(line, max as usize) {
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
                queue!(
                    stdout,
                    cursor::MoveTo(pos.0, pos.1 + idx + 1),
                    Print(checkbox),
                    PrintStyledContent(subline.with(color(idx as usize))),
                )?;
                idx += 1;
            }
        }
        Ok(())
    }

    fn get_max_line_width(&self, size: (u16, u16)) -> usize {
        size.0 as usize - CHECKBOX_WIDTH
    }

    fn get_title(&self) -> PrintStyledContent<&str> {
        PrintStyledContent(match self.list_type {
            ListType::Todo => "Todo".green().bold(),
            ListType::Done => "Done".red().bold(),
        })
    }

    fn get_checkbox(&self) -> String {
        match self.list_type {
            ListType::Todo => "[ ] ".to_string(),
            ListType::Done => format!("[{}] ", "X".red().bold()),
        }
    }

    fn move_to_top(&mut self) {
        self.current_index = 0;
    }

    fn move_to_bottom(&mut self) {
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
        if self.items.is_empty() {
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
        if len == 0 {
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
        if self.items.is_empty() {
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
        let mut index = self.current_index + 1;
        if index > self.items.len() {
            index = self.current_index;
        }
        self.insert(item, index);
    }

    fn set_current(&mut self, item: String) {
        self.items[self.current_index] = item;
    }

    fn clone_current(&mut self) -> Option<String> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items[self.current_index].clone())
        }
    }

    fn get_y_pos(&self, size: (u16, u16)) -> usize {
        let max = self.get_max_line_width(size);
        let mut y = 1; // start at one for title
        // the logic is the position of the current index is the sum
        // is the sum of all the lines before the current line
        // plus 1 for the title offset
        for i in 0..self.current_index {
            y += word_wrap(&self.items[i], max).len();
        }
        y
    }

    fn go_to_current_index(&self, pos: (u16, u16), size: (u16, u16), stdout: &mut io::Stdout) -> crossterm::Result<()> {
        let y = self.get_y_pos(size).checked_sub(self.y_offset).unwrap_or(1) as u16;
        queue!(
            stdout,
            cursor::MoveTo(pos.0, pos.1 + y)
        )
    }

    fn update_y_offset(&mut self, size: (u16, u16)) {
        let y = self.get_y_pos(size);
        self.y_offset = if y > self.y_offset {
            (y + 1).saturating_sub(size.1 as usize)
        } else {
            y - 1
        }
    }

    fn out_of_bounds(&self, size: (u16, u16)) -> bool {
        let y = self.get_y_pos(size);
        y + 1 > size.1 as usize + self.y_offset || y <= self.y_offset
    }

    fn sort(&mut self) {
        self.items.sort();
    }
}

struct TodoApp {
    running: bool,
    stdout: io::Stdout,
    todo: List,
    done: List,
    list_type: ListType,
    input_mode: InputMode,
    input_string: String,
    input_string_index: usize,
    terminal_size: (u16, u16),
    one_pane: bool,
}

impl TodoApp {
    fn new() -> Self {
        let terminal_size = terminal::size().expect("Could not get terminal size");
        Self {
            running: true,
            stdout: io::stdout(),
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

    fn go_to_current_index(&mut self) -> crossterm::Result<()> {
        let size = if self.one_pane {
            self.terminal_size
        } else {
            (self.terminal_size.0 / 2, self.terminal_size.1)
        };
        match self.list_type {
            ListType::Todo => self.todo.go_to_current_index(
                (0, 0),
                size,
                &mut self.stdout,
            ),
            ListType::Done => self.done.go_to_current_index(
                if self.one_pane {
                    (0, 0)
                } else {
                    (self.terminal_size.0 / 2, 0)
                },
                size,
                &mut self.stdout,
            ),
        }
    }

    fn redraw(&mut self) -> crossterm::Result<()> {
        self.clear()?;
        match self.input_mode {
            InputMode::Normal => {
                if self.one_pane {
                    match self.list_type {
                        ListType::Todo => self.draw_todo(),
                        ListType::Done => self.draw_done(),
                    }?
                } else {
                    self.draw_todo()?;
                    self.draw_done()?;
                }
                self.go_to_current_index()?;
            }
            InputMode::Insert(dest) => {
                let leader = PrintStyledContent(match dest {
                    InputDestination::NewItem => "New item: ".blue().bold(),
                    InputDestination::NewItemBefore => "New item before current: ".magenta().bold(),
                    InputDestination::NewItemAfter => "New item after current: ".red().bold(),
                    InputDestination::EditItem => "Edit item: ".green().bold(),
                });
                let input = self.input_string.clone(); // appease borrow checker
                let idx = self.input_string_index; // appease borrow checker
                queue!(
                    &mut self.stdout,
                    leader,
                    Print(input),
                    cursor::MoveTo(
                        leader.0.content().len() as u16 + idx as u16,
                        0
                    ),
                )?;
            }
        }
        self.stdout.flush()
    }

    fn run(&mut self) -> crossterm::Result<()> {
        self.redraw()?;
        while self.running {
            if self.kbin()? {
                self.redraw()?;
            }
        }
        save_list(TODO_LIST, &self.todo.items);
        save_list(DONE_LIST, &self.done.items);
        self.clear()
    }

    fn draw_todo(&mut self) -> crossterm::Result<()> {
        self.todo.draw(
            (0, 0),
            if self.one_pane {
                self.terminal_size
            } else {
                (self.terminal_size.0 / 2, self.terminal_size.1)
            },
            &mut self.stdout
        )
    }

    fn draw_done(&mut self) -> crossterm::Result<()> {
        self.done.draw(
            if self.one_pane {
                (0, 0)
            } else {
                (self.terminal_size.0 / 2, 0)
            },
            if self.one_pane {
                self.terminal_size
            } else {
                (self.terminal_size.0 / 2, self.terminal_size.1)
            },
            &mut self.stdout
        )
    }

    fn clear(&mut self) -> crossterm::Result<()> {
        queue!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )
    }

    fn swap_list(&mut self) {
        self.list_type = self.list_type.next();
    }

    fn check_item(&mut self) {
        if let Some(item) = self.todo.remove() {
            self.done.add(item);
        }
    }

    fn uncheck_item(&mut self) {
        if let Some(item) = self.done.remove() {
            self.todo.add(item);
        }
    }

    fn delete_item(&mut self) {
        self.done.remove();
    }

    /// handle keyboard input
    /// returns Ok(true) if redraw needs to be called again, otherwise returns Ok(false)
    fn kbin(&mut self) -> crossterm::Result<bool> {
        if event::poll(Duration::from_millis(50))? {
            let evnt = event::read()?;
            let list = match self.list_type {
                ListType::Todo => &mut self.todo,
                ListType::Done => &mut self.done,
            };
            match self.input_mode {
                InputMode::Normal => match evnt {
                    Event::Resize(w, h) => {
                        self.terminal_size = (w, h);
                        self.one_pane = self.terminal_size.0 <= MAX_WIDTH_SINGLE_PANE;
                    }
                    Event::Key(key_event) => match (key_event.code, self.list_type) {
                        (KeyCode::Char('q') | KeyCode::Esc, _) => self.running = false,
                        (KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Enter, ListType::Todo) => self.check_item(),
                        (KeyCode::Char('x') | KeyCode::Enter, ListType::Done) => self.uncheck_item(),
                        (KeyCode::Char('d') | KeyCode::Backspace, ListType::Done) => self.delete_item(),
                        (KeyCode::Char('O'), ListType::Todo) => self.input_mode = InputMode::Insert(InputDestination::NewItemBefore),
                        (KeyCode::Char('o'), ListType::Todo) => self.input_mode = InputMode::Insert(InputDestination::NewItemAfter),
                        (KeyCode::Char(ch), _) => match ch {
                            'e' => {
                                self.input_mode = InputMode::Insert(InputDestination::EditItem);
                                if let Some(item) = list.clone_current() {
                                    self.input_string_index = item.len();
                                    self.input_string = item;
                                }
                            }
                            'a' | 'i' => self.input_mode = InputMode::Insert(InputDestination::NewItem),
                            'h' | 'l' => self.swap_list(),
                            'j' => list.move_down(),
                            'J' => list.shift_down(),
                            'k' => list.move_up(),
                            'K' => list.shift_up(),
                            'g' => list.move_to_top(),
                            'G' => list.move_to_bottom(),
                            's' => list.sort(),
                            _ => return Ok(false),
                        }
                        _ => return Ok(false),
                    }
                    _ => return Ok(false),
                },
                InputMode::Insert(dest) => match evnt {
                    Event::Key(key_event) => match key_event.code {
                        KeyCode::Left => if self.input_string_index >= 1 {
                            self.input_string_index -= 1;
                        },
                        KeyCode::Right => {
                            let len = self.input_string.len();
                            self.input_string_index += 1;
                            if self.input_string_index > len {
                                self.input_string_index = len;
                            }
                        }
                        KeyCode::Esc => {
                            self.input_mode = InputMode::Normal;
                            self.input_string = "".to_string();
                            self.input_string_index = 0;
                        }
                        KeyCode::Backspace => {
                            if !self.input_string.is_empty() {
                                if self.input_string_index >= 1 {
                                    self.input_string_index -= 1;
                                }
                                self.input_string.remove(self.input_string_index);
                            }
                        }
                        KeyCode::Enter => {
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
                        }
                        KeyCode::Char(ch) => {
                            self.input_string.insert(self.input_string_index, ch);
                            self.input_string_index += 1;
                        }
                        _ => return Ok(false),
                    },
                    _ => return Ok(false),
                }
            }
        }
        Ok(true)
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
            s            ->  Sort a list
        INSERT MODE:
            Esc          ->  Exit insert mode
            Enter        ->  Add writen todo to list
            Backspace    ->  Remove a character from the label
            other keys   ->  write label for todo"#
)]
struct Args {
    /// Directly add an item to the todo list
    #[structopt(short, long)]
    add: Option<String>,

    /// Directly add an item read from stdin into the todo list
    #[structopt(short="s", long)]
    add_stdin: bool,

    /// Print list instead of interactive prompt
    #[structopt(short, long)]
    print: bool,

    /// Print done list instead of interactive prompt
    #[structopt(short="d", long)]
    print_done: bool,
}

fn main() -> crossterm::Result<()> {
    let mut stdin = io::stdin();
    let args = Args::from_args();
    let stdin_tty = stdin.is_tty();
    let stdout_tty = io::stdout().is_tty();
    // interactive
    if args.add.is_none()
        && !args.add_stdin
        && !args.print
        && !args.print_done
        && stdin_tty
        && stdout_tty
    {
        terminal::enable_raw_mode()?;
        TodoApp::new().run()?;
        terminal::disable_raw_mode()?;
    } else {
        if args.add.is_some() || args.add_stdin || !stdin_tty {
            let mut list = load_list(TODO_LIST);
            if let Some(val) = args.add {
                list.push(val);
            } else {
                let mut val = "".to_string();
                stdin.read_to_string(&mut val)?;
                for line in val.split('\n').filter(|x| !x.is_empty()) {
                    list.push(line.trim().to_string());
                }
            };
            save_list(TODO_LIST, &list);
        }
        if args.print_done {
            print_done();
        } else if args.print || !stdout_tty {
            print_todo();
        }
    }
    Ok(())
}
