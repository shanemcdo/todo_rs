use colored::*;
use structopt::StructOpt;
use std::io::{
    self,
    BufRead,
    prelude::*,
};

fn save_list(filename: &str, list: Vec<String>) {
    let mut file = std::fs::File::open(filename)
        .unwrap_or_else(|_| std::fs::File::create(filename).expect("Could not create file"));
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

fn main() {
    const TODO_LIST: &str = std::env!("TODO_LIST");
    const DONE_LIST: &str = std::env!("TODO_DONE_LIST");
    println!("{:?}", load_list(TODO_LIST));
    println!("{:?}", load_list(DONE_LIST));
}
