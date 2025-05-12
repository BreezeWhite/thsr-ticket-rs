use clap::Parser;

use thsr::cli::Args;
use thsr::run;
use thsr::schema::{STATION_MAP, TIME_TABLE};

fn show_station() {
    for (i, station) in STATION_MAP.iter().enumerate() {
        println!("{}: {:?}", i + 1, station);
    }
}

fn show_time_table() {
    for (idx, &t_str) in TIME_TABLE.iter().enumerate() {
        let mut t_int = t_str[..t_str.len() - 1].parse::<u16>().unwrap();
        if t_str.ends_with('A') && (t_int / 100) == 12 {
            t_int %= 1200;
        } else if t_int != 1230 && t_str.ends_with('P') {
            t_int += 1200;
        }
        let formatted_time = format!("{:04}", t_int);
        println!(
            "{}. {}:{}",
            idx + 1,
            &formatted_time[..formatted_time.len() - 2],
            &formatted_time[formatted_time.len() - 2..]
        );
    }
}

fn main() {
    let args = Args::parse();

    if args.list_time_table {
        show_time_table();
        return;
    }

    if args.list_station {
        show_station();
        return;
    }

    run(args);
}
