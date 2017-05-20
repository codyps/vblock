extern crate hash_roll;
#[macro_use]
extern crate clap;
extern crate rand;

use clap::{Arg, SubCommand};
use rand::Rng;
use hash_roll::Split2;

fn main() {
    let matches = app_from_crate!()
        .subcommand(SubCommand::with_name("bench-split")
                    .about("Benchmark block splitting mechanisms (speed & deduplication), reads data from stdin by default")
                    .arg(Arg::with_name("input-random")
                         .short("r")
                         .value_name("BYTES")
                         .takes_value(true)
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
            if let Some(bytes) = sub_m.value_of("input-random") {
                let mut bytes = match bytes.parse::<usize>() {
                    Err(e) => {
                        eprintln!("--input-random requires a unsigned number of bytes, got '{:?}': {}", bytes, e);
                        std::process::exit(1);
                    },
                    Ok(x) => { x },
                };
                println!("Benchmarking {} bytes of random data", bytes);
                // FIXME: choose better buf size? move into box? allow variation?
                let mut rng = rand::thread_rng();
                let mut buf = [0u8;4096];
                let mut hr = hash_roll::bup::BupBuf::default();

                while bytes > buf.len() {
                    rng.fill_bytes(&mut buf[..]);
                    hr.push(&buf[..]);
                    bytes -= buf.len();
                }

                rng.fill_bytes(&mut buf[0..bytes]);
                hr.push(&buf[0..bytes]);

            } else if let Some(in_file) = sub_m.value_of("input-file") {
                eprintln!("input-file: {}", in_file);
            } else if let Some(in_dir) = sub_m.value_of("input-dir") {
                eprintln!("input-dir: {}", in_dir);
            } else {
                eprintln!("bench-split: {:?}", sub_m);
            }
        },
        (n, _) => {
            eprintln!("Error: unknown SubCommand {:?}", n);
            ::std::process::exit(1);
        }
    }
}
