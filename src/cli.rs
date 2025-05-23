use clap::Parser;
use clap::builder::TypedValueParser;

/// A CLI tool for booking Taiwan High Speed Rail tickets.
/// Run the program without flags will guide you through the booking process.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Personal ID
    #[arg(long, short = 'i', value_name = "ID")]
    pub personal_id: Option<String>,

    /// Departure date
    #[arg(long, short = 'd', value_name = "DATE")]
    pub date: Option<String>,

    /// Time ID of the departure time.
    /// To see available times, use the --list-time-table option.
    #[arg(long, short = 'T', value_name = "TIME_ID")]
    pub time: Option<usize>,

    /// Departure station ID.
    /// To see available stations, use the --list-station option.
    #[arg(long, short = 'f', value_name = "STATION_ID")]
    pub from: Option<usize>,

    /// Arrival station ID.
    /// To see available stations, use the --list-station option.
    #[arg(long, short = 't', value_name = "STATION_ID")]
    pub to: Option<usize>,

    /// Number of adults
    #[arg(long, short = 'a', value_name = "NUMBER")]
    pub adult_cnt: Option<u8>,

    /// Number of students
    #[arg(long, short = 's', value_name = "NUMBER")]
    pub student_cnt: Option<u8>,

    /// Seat preference. 0: None, 1: Window, 2: Aisle
    #[arg(
        long,
        short = 'p',
        value_name = "NUMBER",
        value_parser = clap::builder::PossibleValuesParser::new(["0", "1", "2"])
            .map(|s| s.parse::<usize>().unwrap())
        )
    ]
    pub seat_prefer: Option<usize>,

    /// Class type. 0: Standard, 1: Business
    #[arg(
        long,
        short = 'c',
        value_name = "NUMBER",
        value_parser = clap::builder::PossibleValuesParser::new(["0", "1"])
            .map(|s| s.parse::<usize>().unwrap())
        )
    ]
    pub class_type: Option<usize>,

    /// Whether to use personal ID as membership
    #[arg(long, short = 'm', value_name = "TO_USE_MEMBERSHIP")]
    pub use_membership: Option<bool>,

    /// List available stations
    #[arg(long)]
    pub list_station: bool,

    /// List available times
    #[arg(long)]
    pub list_time_table: bool,
}
