// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

use std::{env, io::Write, os::unix::net::UnixStream, path::PathBuf, process};

use daemon_generated::{
    CTL, SOCKET,
    error::strerror,
    imsg,
    ipc::{Request, Response, Status},
};

fn usage() -> ! {
    eprintln!("usage: {CTL} [-s socket] command [argument ...]");
    process::exit(1);
}

#[derive(Clone, Copy)]
enum Action {
    None,
    ShowStatus,
    LogVerbose,
    LogBrief,
    Reload,
    Stop,
}

/// One keyword; `next` is the table to match the following argument
/// against, empty when the command is complete.
struct Token {
    keyword: &'static str,
    action: Action,
    next: &'static [Token],
}

const T_MAIN: &[Token] = &[
    Token {
        keyword: "show",
        action: Action::None,
        next: T_SHOW,
    },
    Token {
        keyword: "log",
        action: Action::None,
        next: T_LOG,
    },
    Token {
        keyword: "reload",
        action: Action::Reload,
        next: &[],
    },
    Token {
        keyword: "stop",
        action: Action::Stop,
        next: &[],
    },
];

const T_SHOW: &[Token] = &[Token {
    keyword: "status",
    action: Action::ShowStatus,
    next: &[],
}];

const T_LOG: &[Token] = &[
    Token {
        keyword: "verbose",
        action: Action::LogVerbose,
        next: &[],
    },
    Token {
        keyword: "brief",
        action: Action::LogBrief,
        next: &[],
    },
];

fn show_valid(table: &[Token]) {
    eprintln!("valid commands/args:");
    for t in table {
        eprintln!("  {}", t.keyword);
    }
}

/// Prefix-match `args` against the keyword tables.
fn parse(args: &[String]) -> Action {
    let mut table = T_MAIN;
    let mut action = Action::None;
    let mut i = 0;
    while !table.is_empty() {
        let Some(word) = args.get(i) else {
            eprintln!("{CTL}: missing argument");
            show_valid(table);
            process::exit(1);
        };
        let exact = table.iter().find(|t| t.keyword == word);
        let matched = match exact {
            Some(t) => t,
            None => {
                let mut it = table
                    .iter()
                    .filter(|t| t.keyword.starts_with(word.as_str()));
                match (it.next(), it.next()) {
                    (Some(t), None) => t,
                    (Some(_), Some(_)) => {
                        eprintln!("{CTL}: ambiguous argument: {word}");
                        show_valid(table);
                        process::exit(1);
                    }
                    (None, _) => {
                        eprintln!("{CTL}: unknown argument: {word}");
                        show_valid(table);
                        process::exit(1);
                    }
                }
            }
        };
        action = matched.action;
        table = matched.next;
        i += 1;
    }
    if i < args.len() {
        eprintln!("{CTL}: unknown argument: {}", args[i]);
        process::exit(1);
    }
    action
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut p = getopt::Parser::new(&args, "s:");
    let mut socket: PathBuf = SOCKET.into();
    loop {
        match p.next().transpose() {
            Ok(None) => break,
            Ok(Some(getopt::Opt('s', Some(s)))) => socket = s.into(),
            Ok(Some(_)) => unreachable!(),
            Err(e) => {
                eprintln!("{CTL}: {e}");
                usage();
            }
        }
    }
    let rest = &args[p.index()..];
    if rest.is_empty() {
        usage();
    }

    let req = match parse(rest) {
        Action::None => usage(),
        Action::ShowStatus => Request::ShowStatus,
        Action::LogVerbose => Request::LogVerbose(true),
        Action::LogBrief => Request::LogVerbose(false),
        Action::Reload => Request::Reload,
        Action::Stop => Request::Shutdown,
    };

    let mut stream = UnixStream::connect(&socket).unwrap_or_else(|e| {
        eprintln!("{CTL}: connect: {}: {}", socket.display(), strerror(&e));
        process::exit(1);
    });
    let r = req
        .encode()
        .and_then(|buf| stream.write_all(&buf).and_then(|()| stream.flush()));
    if let Err(e) = r {
        eprintln!("{CTL}: write: {}", strerror(&e));
        process::exit(1);
    }
    let resp = match imsg::read_blocking(&mut stream) {
        Ok(m) => Response::from_imsg(&m).unwrap_or_else(|e| {
            eprintln!("{CTL}: {e}");
            process::exit(1);
        }),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            eprintln!("{CTL}: pipe closed");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("{CTL}: read: {}", strerror(&e));
            process::exit(1);
        }
    };

    match resp {
        Response::Status(s) => print_status(&s),
        Response::Ok => println!("command succeeded"),
        Response::Fail(msg) => {
            eprintln!("{CTL}: {msg}");
            process::exit(1);
        }
    }
}

fn print_status(s: &Status) {
    println!("pid:       {}", s.pid);
    println!("uptime:    {}", uptime(s.uptime_secs));
    println!("verbose:   {}", if s.verbose { "yes" } else { "no" });
    println!("reloads:   {}", s.reloads);
    println!("socket:    {}", s.config.socket.display());
    println!("user:      {}", s.config.user.as_deref().unwrap_or("-"));
    println!(
        "chroot:    {}",
        s.config
            .chroot
            .as_deref()
            .map_or("-".into(), |p| p.display().to_string())
    );
    println!("interval:  {}s", s.config.interval);
}

fn uptime(secs: u64) -> String {
    let (d, h, m, s) =
        (secs / 86400, secs / 3600 % 24, secs / 60 % 60, secs % 60);
    if d > 0 {
        format!("{d}d{h:02}h{m:02}m{s:02}s")
    } else if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else if m > 0 {
        format!("{m}m{s:02}s")
    } else {
        format!("{s}s")
    }
}
