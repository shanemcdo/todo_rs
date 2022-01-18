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
}

impl ListApp{
    fn new() -> Self{
        Self {
            running: true,
            stdin: termion::async_stdin().keys(),
            stdout: io::stdout().into_raw_mode().unwrap(),
            todo: load_list(TODO_LIST),
            done: load_list(DONE_LIST),
        }
    }

    fn redraw(&mut self){
        self.clear();
        self.draw_done();
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

    fn kbin(&mut self) -> bool {
        if let Some(Ok(key)) = self.stdin.next() {
            match key {
                termion::event::Key::Char(ch) => match ch {
                    'q' => self.running = false,
                    'h' => (),
                    'j' => (),
                    'k' => (),
                    'l' => (),
                    _ => return false,
                }
                _ => return false,
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
    }
    ListApp::new().run();
}
