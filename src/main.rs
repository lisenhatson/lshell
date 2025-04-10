use std::env;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio, Child};
use std::os::unix::io::AsRawFd;

use libc::{tcgetattr, tcsetattr, termios, TCSANOW, ICANON, ECHO};

fn set_raw_mode(fd: i32, enable: bool, original_term: &mut termios) {
    unsafe {
        if enable {
            tcgetattr(fd, original_term);
            let mut raw = *original_term;
            raw.c_lflag &= !(ICANON | ECHO);
            tcsetattr(fd, TCSANOW, &raw);
        } else {
            tcsetattr(fd, TCSANOW, original_term);
        }
    }
}

fn main() {
    let mut history: Vec<String> = Vec::new();
    let mut history_index: Option<usize> = None;

    let stdin_fd = io::stdin().as_raw_fd();
    let mut original_term = termios {
        c_iflag: 0,
        c_oflag: 0,
        c_cflag: 0,
        c_lflag: 0,
        c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0,
        c_ospeed: 0,
    };

    loop {
        print!("$_ ");
        io::stdout().flush().unwrap();

        let mut _buffer: Vec<String> = Vec::new();
        let mut input = String::new();
        let mut byte = [0; 1];

        set_raw_mode(stdin_fd, true, &mut original_term);

        while io::stdin().read(&mut byte).unwrap() > 0 {
            match byte[0] {
                b'\n' => {
                    println!();
                    break;
                }
                0x1B => {
                    let mut seq = [0; 2];
                    io::stdin().read_exact(&mut seq).unwrap();
                    if seq == [91, 65] {
                        // Up arrow
                        if let Some(idx) = history_index {
                            if idx > 0 {
                                history_index = Some(idx - 1);
                            }
                        } else if !history.is_empty() {
                            history_index = Some(history.len() - 1);
                        }

                        if let Some(idx) = history_index {
                            input.clear();
                            input.push_str(&history[idx]);
                            print!("\r$_ {}", input);
                            print!("\x1B[K"); // Clear to end of line
                            io::stdout().flush().unwrap();
                        }
                    } else if seq == [91, 66] {
                        // Down arrow
                        if let Some(idx) = history_index {
                            if idx + 1 < history.len() {
                                history_index = Some(idx + 1);
                                input.clear();
                                input.push_str(&history[history_index.unwrap()]);
                            } else {
                                history_index = None;
                                input.clear();
                            }
                            print!("\r$_ {}", input);
                            print!("\x1B[K"); // Clear to end of line
                            io::stdout().flush().unwrap();
                        }
                    }
                }
                127 => {
                    // Backspace
                    if !input.is_empty() {
                        input.pop();
                        print!("\r$_ {}", input);
                        print!("\x1B[K");
                        io::stdout().flush().unwrap();
                    }
                }
                _ => {
                    input.push(byte[0] as char);
                    print!("{}", byte[0] as char);
                    io::stdout().flush().unwrap();
                }
            }
        }

        set_raw_mode(stdin_fd, false, &mut original_term);

        if input.trim().is_empty() {
            continue;
        }

        history.push(input.clone());
        history_index = None;

        let mut commands = input.trim().split(" | ").peekable();
        let mut previous_command = None;

        while let Some(command) = commands.next() {
            let mut parts = command.trim().split_whitespace();
            let command = parts.next().unwrap();
            let args = parts;

            match command {
                "cd" => {
                    let new_dir = args.peekable().peek().map_or("/", |x| *x);
                    let root = Path::new(new_dir);
                    if let Err(e) = env::set_current_dir(&root) {
                        eprintln!("{}", e);
                    }
                    previous_command = None;
                }
                "exit" => return,
                command => {
                    let stdin = previous_command
                        .map_or(Stdio::inherit(), |output: Child| {
                            Stdio::from(output.stdout.unwrap())
                        });

                    let stdout = if commands.peek().is_some() {
                        Stdio::piped()
                    } else {
                        Stdio::inherit()
                    };

                    let output = Command::new(command)
                        .args(args)
                        .stdin(stdin)
                        .stdout(stdout)
                        .spawn();

                    match output {
                        Ok(output) => previous_command = Some(output),
                        Err(e) => {
                            previous_command = None;
                            eprintln!("{}", e);
                        }
                    }
                }
            }
        }

        if let Some(mut final_command) = previous_command {
            let _ = final_command.wait();
        }
    }
}
