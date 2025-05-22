# THSR-Ticket Rust

A program for booking ticket of Taiwan High Speed Railway (THSR).

This is the rust version of the original python ver [THSR-Ticket](https://github.com/BreezeWhite/THSR-Ticket), with improvements including more CLI input options, early-bird support, membership, etc.

## Install
### Download from the [Release](https://github.com/BreezeWhite/thsr-ticket-rs/releases) page

Pick the one according to your OS, download the executable file, and open the terminal to execute it.

You could also put it to one of your PATH dir (e.g. ~/.local/bin for Ubuntu) to always make it accessable.

### Install with cargo

Make sure you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed.

For older linux machines, this method is preferred, since the release is built by Github Action and the workflow only supports Ubuntu-22.04 and latter.

```bash
cargo install --git https://github.com/BreezeWhite/thsr-ticket-rs
```

## Usage

```bash
# Use without flags.
# This will guide you through the process for entering informations.
thsr

# Or pass values to arguments.
# If some required informations are not specified, the program will ask you to enter.
thsr --from 2 --to 11 --adult-cnt 2

# To see available stations and its ID value
thsr --list-station

# To see available times and its ID value
thsr --list-time-table

# All following date formats are supported
thsr --date 2025/01/01
thsr --date 2025/1/01
thsr --date 2025/01/1
thsr --date 2025/1/1

# To use membership, you need to explicitly specify the flag `--membership-id`.
# The default booking process does not include entering membership.
thsr --membership-id <your_id_here>
```

### Complete options

```bash
Usage: thsr [OPTIONS]

Options:
  -i, --personal-id <ID>               Personal ID
  -d, --date <DATE>                    Departure date
  -T, --time <TIME_ID>                 Time ID of the departure time. To see available times, use the --list-time-table option
  -f, --from <STATION_ID>              Departure station ID. To see available stations, use the --list-station option
  -t, --to <STATION_ID>                Arrival station ID. To see available stations, use the --list-station option
  -a, --adult-cnt <NUMBER>             Number of adults
  -s, --student-cnt <NUMBER>           Number of students
  -p, --seat-prefer <NUMBER>           Seat preference. 0: None, 1: Window, 2: Aisle [possible values: 0, 1, 2]
  -c, --class-type <NUMBER>            Class type. 0: Standard, 1: Business [possible values: 0, 1]
  -m, --membership-id <MEMBERSHIP_ID>  Use membership ID
      --list-station                   List available stations
      --list-time-table                List available times
  -h, --help                           Print help
  -V, --version                        Print version
```


## ***DISCLAIMER***

This is an unofficial implementation and is for research purpose only. Any legal liability is on your own. Use at your own risk.
