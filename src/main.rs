#[macro_use]
extern crate clap;

use clap::{Arg, SubCommand};

fn main() {
    let matches = app_from_crate!()
        .subcommand(SubCommand::with_name("bench-split")
                    .about("Benchmark block splitting mechanisms (speed & deduplication), reads data from stdin by default")
                    .arg(Arg::with_name("input-random")
                         .short("r")
                         .help("Use random data for benchmark")
                         .conflicts_with_all(&["input-file", "input-dir"]))
                    .arg(Arg::with_name("input-file")
                         .short("f")
                         .help("Use a file for data")
                         .value_name("FILE")
                         .takes_value(true)
                         .conflicts_with_all(&["input-random", "input-dir"]))
                    .arg(Arg::with_name("input-dir")
                         .short("d")
                         .help("Use the contents of a directory (recursively) for data")
                         .value_name("DIR")
                         .takes_value(true)
                         .conflicts_with_all(&["input-random", "input-file"]))
        ).get_matches();


    match matches.subcommand() {
        ("bench-split", Some(sub_m)) => {
            let rand = sub_m.is_present("input-random");
            println!("bench-split: {:?}", sub_m);
        },
        (n, _) => {
            eprintln!("Error: unknown SubCommand {:?}", n);
            ::std::process::exit(1);
        }
    }
}
