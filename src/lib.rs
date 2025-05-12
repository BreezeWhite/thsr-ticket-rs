pub mod cli;
pub mod schema;

use bytes::Bytes;
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use strum::IntoEnumIterator;

use crate::cli::Args;
use crate::schema::{StationMap, TIME_TABLE, TicketType};

static BASE_URL: &str = "https://irs.thsrc.com.tw";
static BOOKING_PAGE_URL: &str = "https://irs.thsrc.com.tw/IMINT/?locale=tw";
static SUBMIT_FORM_URL: &str = "https://irs.thsrc.com.tw/IMINT/;jsessionid={}?wicket:interface=:0:BookingS1Form::IFormSubmitListener";
static CONFIRM_TRAIN_URL: &str =
    "https://irs.thsrc.com.tw/IMINT/?wicket:interface=:1:BookingS2Form::IFormSubmitListener";
static CONFIRM_TICKET_URL: &str =
    "https://irs.thsrc.com.tw/IMINT/?wicket:interface=:2:BookingS3Form::IFormSubmitListener";

fn get_header() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Host", HeaderValue::from_static("irs.thsrc.com.tw"));
    headers.insert(
        "User-Agent",
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:137.0) Gecko/20100101 Firefox/137.0",
        ),
    );
    headers.insert(
        "Accept",
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("zh-TW,zh;q=0.8,en-US;q=0.5,en;q=0.3"),
    );
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert("Connection", HeaderValue::from_static("keep-alive"));
    headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://irs.thsrc.com.tw/IMINT/"),
    );
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("no-cors"));
    headers
}

pub fn run(args: Args) {
    let policy = reqwest::redirect::Policy::limited(20);
    let client = Client::builder()
        .redirect(policy)
        .default_headers(get_header())
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap();

    // First page
    let resp = match booking_flow::run_flow(&client, &args) {
        Ok(resp) => resp,
        Err(err_msg) => {
            println!("Error: {}", err_msg);
            return;
        }
    };

    // Second Page
    let resp = match confirm_train_flow::run_flow(resp, &client) {
        Ok(resp) => resp,
        Err(err_msg) => {
            println!("Error: {}", err_msg);
            return;
        }
    };

    // Final page
    let resp = match confirm_ticket_flow::run_flow(&resp, &client, &args) {
        Ok(resp) => resp,
        Err(err_msg) => {
            println!("Error: {}", err_msg);
            return;
        }
    };

    // Show the final booking result
    show_result(&resp);
}

pub fn parse_error(page: &Html) -> Option<String> {
    let err_selector = Selector::parse("span.feedbackPanelERROR").unwrap();
    let errors: Vec<String> = page
        .select(&err_selector)
        .filter_map(|element| element.text().next().map(|text| text.trim().to_string()))
        .collect();
    if errors.is_empty() {
        None
    } else {
        Some(errors.join("\n"))
    }
}

// First page: Booking Flow
pub mod booking_flow {
    use super::*;

    pub fn run_flow(client: &Client, args: &Args) -> Result<Html, String> {
        println!("Requesting booking page...");
        let response = client.get(BOOKING_PAGE_URL).send().unwrap();

        // Parse jsession id
        let jid = response
            .cookies()
            .find(|cookie| cookie.name() == "JSESSIONID")
            .map(|cookie| cookie.value().to_string())
            .unwrap();

        // Parse to HTML object
        let body = response.text().unwrap(); // Get the response body as a string
        let document = Html::parse_document(&body);

        // Request sercurity code image
        let sec_code_img_url = parse_sercurity_code_img_url(&document);
        let img_resp = client.get(&sec_code_img_url).send().unwrap();

        // Making selections
        let mut payload = BookingPayload::default();
        payload.search_by = parse_search_by(&document);
        payload.types_of_trip = parse_types_of_trip_value(&document);
        payload.select_start_station(&args.from);
        payload.select_dest_station(&args.to);
        let (start_date, end_date) = parse_avail_start_end_date(&document);
        payload.select_date(&start_date, &end_date, &args.date);
        payload.select_time(&args.time);
        if args.adult_cnt.is_none() && args.student_cnt.is_none() {
            payload.select_ticket_num(TicketType::Adult, &None);
        }
        if args.adult_cnt.is_some() {
            payload.select_ticket_num(TicketType::Adult, &args.adult_cnt);
        }
        if args.student_cnt.is_some() {
            payload.select_ticket_num(TicketType::College, &args.student_cnt);
        }
        payload.select_seat_prefer(&args.seat_prefer);
        payload.select_class_type(&args.class_type);
        payload.input_security_code(img_resp.bytes().unwrap());

        // Make the booking request
        let resp = client
            .post(SUBMIT_FORM_URL.replace("{}", &jid))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(serde_urlencoded::to_string(&payload).unwrap())
            .send()
            .unwrap();

        // Parse to HTML object
        let resp_html = Html::parse_document(&resp.text().unwrap());
        if let Some(err_msg) = parse_error(&resp_html) {
            return Err(err_msg);
        }
        Ok(resp_html)
    }

    fn parse_avail_start_end_date(page: &Html) -> (String, String) {
        let selector = Selector::parse("#toTimeInputField").unwrap();
        let elem = page.select(&selector).next().unwrap();
        let end_date = elem.attr("limit").unwrap();
        let start_date = elem.attr("date").unwrap();
        (start_date.to_string(), end_date.to_string())
    }

    fn parse_types_of_trip_value(page: &Html) -> u8 {
        let selector = Selector::parse("#BookingS1Form_tripCon_typesoftrip").unwrap();
        let elem = page.select(&selector).next().unwrap();
        let selected_selector = Selector::parse("[selected='selected']").unwrap();
        let trip_type = elem.select(&selected_selector).next().unwrap();
        trip_type.attr("value").unwrap().parse().unwrap()
    }

    fn parse_search_by(page: &Html) -> String {
        let candidates_selector = Selector::parse("input[name='bookingMethod']").unwrap();
        let candidates = page.select(&candidates_selector);
        let tag = candidates
            .filter(|cand| cand.value().attr("checked").is_some())
            .next()
            .unwrap();
        tag.value().attr("value").unwrap().to_string()
    }

    fn parse_sercurity_code_img_url(page: &Html) -> String {
        let selector = Selector::parse("#BookingS1Form_homeCaptcha_passCode").unwrap();
        let elem = page.select(&selector).next().unwrap();
        let img_url = elem.attr("src").unwrap();
        format!("{}{}", BASE_URL, img_url)
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct BookingPayload {
        #[serde(rename(serialize = "selectStartStation"))]
        pub start_station: u8,

        #[serde(rename(serialize = "selectDestinationStation"))]
        pub dest_station: u8,

        #[serde(rename(serialize = "bookingMethod"))]
        pub search_by: String,

        #[serde(rename(serialize = "tripCon:typesoftrip"), default)]
        pub types_of_trip: u8, // 0: one way, 1: round trip

        #[serde(rename(serialize = "toTimeInputField"))]
        pub outbound_date: String,

        #[serde(rename(serialize = "toTimeTable"))]
        pub outbound_time: String,

        #[serde(rename(serialize = "homeCaptcha:securityCode"))]
        pub security_code: String,

        #[serde(rename(serialize = "seatCon:seatRadioGroup"))]
        pub seat_prefer: usize, // 0: any, 1: window, 2: aisle

        #[serde(rename(serialize = "BookingS1Form:hf:0"), default)]
        pub form_mark: String,

        #[serde(rename(serialize = "trainCon:trainRadioGroup"), default)]
        pub class_type: u8, // 0: standard, 1: business

        #[serde(rename(serialize = "backTimeInputField"))]
        pub inbound_date: Option<String>,

        #[serde(rename(serialize = "backTimeTable"))]
        pub inbound_time: Option<String>,

        #[serde(rename(serialize = "toTrainIDInputField"), default)]
        pub to_train_id: Option<u8>,

        #[serde(rename(serialize = "backTrainIDInputField"), default)]
        pub back_train_id: Option<u8>,

        #[serde(
            rename(serialize = "ticketPanel:rows:0:ticketAmount"),
            default = "default_adult_ticket_num"
        )]
        pub adult_ticket_num: String,

        #[serde(
            rename(serialize = "ticketPanel:rows:1:ticketAmount"),
            default = "default_child_ticket_num"
        )]
        pub child_ticket_num: String,

        #[serde(
            rename(serialize = "ticketPanel:rows:2:ticketAmount"),
            default = "default_disabled_ticket_num"
        )]
        pub disabled_ticket_num: String,

        #[serde(
            rename(serialize = "ticketPanel:rows:3:ticketAmount"),
            default = "default_elder_ticket_num"
        )]
        pub elder_ticket_num: String,

        #[serde(
            rename(serialize = "ticketPanel:rows:4:ticketAmount"),
            default = "default_college_ticket_num"
        )]
        pub college_ticket_num: String,
    }

    pub fn default_adult_ticket_num() -> String {
        "1F".to_string()
    }

    pub fn default_child_ticket_num() -> String {
        "0H".to_string()
    }

    pub fn default_disabled_ticket_num() -> String {
        "0W".to_string()
    }

    pub fn default_elder_ticket_num() -> String {
        "0E".to_string()
    }

    pub fn default_college_ticket_num() -> String {
        "0P".to_string()
    }

    impl Default for BookingPayload {
        fn default() -> Self {
            BookingPayload {
                start_station: StationMap::Nangang as u8,
                dest_station: StationMap::Zuouing as u8,
                search_by: "1".to_string(),
                types_of_trip: 0,
                outbound_date: "2023/10/01".to_string(),
                outbound_time: "08:00".to_string(),
                security_code: "1234".to_string(),
                seat_prefer: 0,
                form_mark: "".to_string(),
                class_type: 0,
                inbound_date: None,
                inbound_time: None,
                to_train_id: None,
                back_train_id: None,
                adult_ticket_num: default_adult_ticket_num(),
                child_ticket_num: default_child_ticket_num(),
                disabled_ticket_num: default_disabled_ticket_num(),
                elder_ticket_num: default_elder_ticket_num(),
                college_ticket_num: default_college_ticket_num(),
            }
        }
    }

    impl BookingPayload {
        pub fn select_start_station(&mut self, from: &Option<usize>) {
            if let Some(from) = from {
                self.start_station = from.clone() as u8;
                return;
            }

            println!("Please select start station:");
            for (i, station) in StationMap::iter().enumerate() {
                println!("{}: {:?}", i + 1, station);
            }
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or_default();
            let input: usize = input.trim().parse().unwrap_or(1);
            if input > 0 && input <= StationMap::iter().count() {
                self.start_station = StationMap::iter().nth(input - 1).unwrap() as u8;
            } else {
                println!("Invalid input, defaulting to Nangang.");
                self.start_station = StationMap::Nangang as u8;
            }
        }

        pub fn select_dest_station(&mut self, to: &Option<usize>) {
            if let Some(to) = to {
                self.dest_station = to.clone() as u8;
                return;
            }

            println!("Please select destination station:");
            for (i, station) in StationMap::iter().enumerate() {
                println!("{}: {:?}", i + 1, station);
            }
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or(12);
            let input: usize = input.trim().parse().unwrap();
            if input > 0 && input <= StationMap::iter().count() {
                self.dest_station = StationMap::iter().nth(input - 1).unwrap() as u8;
            } else {
                println!("Invalid input, defaulting to Zuouing.");
                self.dest_station = StationMap::Zuouing as u8;
            }
        }

        pub fn input_security_code(&mut self, img_data: Bytes) {
            println!("Input sercurity code:");
            show_image(&img_data);
            // Read the security code from the user
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read input");
            self.security_code = input.trim().to_string();
        }

        pub fn select_date(
            &mut self,
            start_date: &String,
            end_date: &String,
            date: &Option<String>,
        ) {
            let input = match date.clone() {
                Some(date) => date,
                None => {
                    println!(
                        "Please select a date between {} and {} (default to {}):",
                        start_date, end_date, start_date
                    );
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap_or_default();
                    let input: String = input.trim().to_string();
                    input
                }
            };

            let input = match normalize_date(&input) {
                Some(date) => date,
                None => {
                    println!("Invalid date format, defaulting to {}", start_date);
                    start_date.clone()
                }
            };

            if input.is_empty() {
                self.outbound_date = start_date.clone();
                return;
            }

            if input.ge(start_date) && input.le(end_date) {
                self.outbound_date = input;
            } else {
                println!("Invalid date, defaulting to {}", start_date);
                self.outbound_date = start_date.to_string();
            }
        }

        pub fn select_time(&mut self, time: &Option<usize>) {
            let opt = match time.clone() {
                Some(time) => time,
                None => {
                    println!("Select departure time:");
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

                    let mut input = String::new();
                    println!("Input selection (default: {}):", 10);
                    std::io::stdin().read_line(&mut input).unwrap_or_default();
                    let selected_opt: usize = input.trim().parse().unwrap_or(10);
                    selected_opt
                }
            };

            if opt > TIME_TABLE.len() {
                println!("Invalid input, defaulting to 10.");
                self.outbound_time = TIME_TABLE[9].to_string();
                return;
            }

            self.outbound_time = TIME_TABLE[opt - 1].to_string();
        }

        pub fn select_ticket_num(&mut self, ticket_type: TicketType, val: &Option<u8>) {
            let mut val = match val.clone() {
                Some(val) => val,
                None => {
                    println!(
                        "Please select the number (0~10) of tickets for {:?} (default: 1)",
                        ticket_type
                    );
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap_or(1);
                    let input: u8 = input.trim().parse().unwrap_or(1);
                    input
                }
            };

            if val > 10 {
                println!("Invalid input, defaulting to 1.");
                val = 1;
            }

            let val = format!("{}{}", val, (ticket_type.clone() as u8) as char);
            match ticket_type {
                TicketType::Adult => self.adult_ticket_num = val,
                TicketType::Child => self.child_ticket_num = val,
                TicketType::Disabled => self.disabled_ticket_num = val,
                TicketType::Elder => self.elder_ticket_num = val,
                TicketType::College => self.college_ticket_num = val,
            }
        }

        pub fn select_seat_prefer(&mut self, prefer: &Option<usize>) {
            let input = match prefer.clone() {
                Some(prefer) => prefer,
                None => {
                    println!(
                        "Please select seat preference (0: any, 1: window, 2: aisle) (default: 0):"
                    );
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap_or_default();
                    let input: usize = input.trim().parse().unwrap_or(0);
                    input
                }
            };

            if input > 2 {
                println!("Invalid input, defaulting to any.");
                self.seat_prefer = 0;
            } else {
                self.seat_prefer = input;
            }
        }

        pub fn select_class_type(&mut self, class_type: &Option<usize>) {
            let input = match class_type.clone() {
                Some(class_type) => class_type,
                None => {
                    println!("Please select class type (0: standard, 1: business) (default: 0):");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap_or_default();
                    let input: usize = input.trim().parse().unwrap_or(0);
                    input
                }
            };

            if input > 1 {
                println!("Invalid input, defaulting to standard.");
                self.class_type = 0;
            } else {
                self.class_type = input as u8;
            }
        }
    }

    fn normalize_date(input: &str) -> Option<String> {
        let re = Regex::new(r"^[1-9]\d{3}/(0?[1-9]|1[0-2])/(0?[1-9]|[12][0-9]|3[01])$").unwrap();
        if let Some(captures) = re.captures(input) {
            let month = captures.get(1).unwrap().as_str().parse::<u8>().unwrap();
            let day = captures.get(2).unwrap().as_str().parse::<u8>().unwrap();
            return Some(format!(
                "{:04}/{:02}/{:02}",
                input[..4].parse::<u16>().unwrap(),
                month,
                day
            ));
        }
        None
    }

    fn show_image(img_data: &[u8]) {
        // Save the image to a file
        let file_name = "tmp_code.jpg";
        fs::write(file_name, img_data).expect("Failed to write image file");

        // Open the image using the default image viewer
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(&["/C", file_name])
                .spawn()
                .expect("Failed to open image");
        } else if cfg!(target_os = "macos") {
            Command::new("open")
                .arg(file_name)
                .spawn()
                .expect("Failed to open image");
        } else if cfg!(target_os = "linux") {
            Command::new("xdg-open")
                .arg(file_name)
                .spawn()
                .expect("Failed to open image");
        } else {
            println!("Please open the image manually: {}", file_name);
        }
    }
}

// Second page: Confirm Train Flow
pub mod confirm_train_flow {
    use super::*;

    pub fn run_flow(document: Html, client: &Client) -> Result<Html, String> {
        // Parse alerts
        let alerts = parse_alert_body(&document);
        println!("{}", alerts.join("\n"));

        // Parse available trains
        let trains = parse_trains(&document);
        let mut payload = ConfirmTrainPayload::default();
        payload.select_available_trains(trains.as_slice());

        let resp = client
            .post(CONFIRM_TRAIN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(serde_urlencoded::to_string(&payload).unwrap())
            .send()
            .unwrap();

        // Parse to HTML object
        let resp_html = Html::parse_document(&resp.text().unwrap());
        if let Some(err_msg) = parse_error(&resp_html) {
            return Err(err_msg);
        }
        Ok(resp_html)
    }

    fn parse_alert_body(document: &Html) -> Vec<String> {
        let li_selector = Selector::parse("ul.alert-body > li").unwrap();
        document
            .select(&li_selector)
            .map(|tag| tag.text().collect::<Vec<_>>().join("").trim().to_string())
            .collect()
    }

    fn parse_trains(document: &Html) -> Vec<Train> {
        let selector = Selector::parse("label.result-item").unwrap(); // Adjust the selector based on `self.cond.from_html`
        let avail = document.select(&selector);

        avail
            .map(|element| {
                let tag_selector = Selector::parse("input").unwrap();
                let elem = element.select(&tag_selector).next().unwrap();

                let id = elem.attr("querycode").unwrap().parse().unwrap();
                let depart = elem.attr("querydeparture").unwrap().to_string();
                let arrive = elem.attr("queryarrival").unwrap().to_string();
                let travel_time = elem.attr("queryestimatedtime").unwrap().to_string();
                let form_value = elem.attr("value").unwrap().to_string();
                let discount_info = parse_discount(&element);

                Train {
                    id,
                    depart,
                    arrive,
                    travel_time,
                    discount_info,
                    form_value,
                }
            })
            .collect()
    }

    fn parse_discount(item: &scraper::ElementRef) -> String {
        let mut discounts = Vec::new();

        if let Some(tag) = item
            .select(&Selector::parse("p.early-bird span").unwrap())
            .next()
        {
            discounts.push(tag.text().next().unwrap().to_string());
        }

        if let Some(tag) = item
            .select(&Selector::parse("p.student span").unwrap())
            .next()
        {
            discounts.push(tag.text().next().unwrap().to_string());
        }

        if !discounts.is_empty() {
            format!("({})", discounts.join(", "))
        } else {
            String::new()
        }
    }

    #[derive(Debug)]
    pub struct Train {
        id: u32,
        depart: String,
        arrive: String,
        travel_time: String,
        discount_info: String,
        form_value: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ConfirmTrainPayload {
        #[serde(rename(serialize = "TrainQueryDataViewPanel:TrainGroup"), default)]
        pub selected_train: String,

        #[serde(rename(serialize = "BookingS2Form:hf:0"), default)]
        pub form_mark: String,
    }

    impl Default for ConfirmTrainPayload {
        fn default() -> Self {
            ConfirmTrainPayload {
                selected_train: "".to_string(),
                form_mark: "".to_string(),
            }
        }
    }

    impl ConfirmTrainPayload {
        pub fn select_available_trains(&mut self, trains: &[Train]) {
            for (idx, train) in trains.iter().enumerate() {
                println!(
                    "{:>2}. {:>4} {:>3}~{} {:>3} {}",
                    idx + 1,
                    train.id,
                    train.depart,
                    train.arrive,
                    train.travel_time,
                    train.discount_info
                );
            }

            println!("Select a train (default: 1):");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or_default();
            let selection: usize = input.trim().parse().unwrap_or(1);

            self.selected_train = trains[selection - 1].form_value.clone();
        }
    }
}

// Final page: Confirm Ticket Flow
pub mod confirm_ticket_flow {
    use super::*;

    pub fn run_flow(document: &Html, client: &Client, args: &Args) -> Result<Html, String> {
        // let body = fs::read_to_string("confirm_response.html").unwrap();
        // let body = std::fs::read_to_string("confirm_ticket_super_early_bird.html").unwrap();

        let mut payload = ConfirmTicketPayload::default();

        // Parse membership radio
        let (radio_value, add_payload) = process_membership(&document, &args.membership_id);
        payload.member_radio = radio_value;

        let personal_id = payload.input_personal_id(&args.personal_id);
        let mut payload = serde_urlencoded::to_string(&payload).unwrap();

        // Additional flow for early bird
        if let Some(additional_payload) = process_early_bird(&document, &personal_id) {
            let additional_payload = serde_urlencoded::to_string(&additional_payload).unwrap();
            payload = format!("{}&{}", payload, additional_payload);
        }

        if let Some(add_payload) = add_payload {
            payload = format!("{}&{}", payload, add_payload);
        }

        println!("Booking...");
        let resp = client
            .post(CONFIRM_TICKET_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(payload)
            .send()
            .unwrap();

        let html = Html::parse_document(&resp.text().unwrap());
        if let Some(err_msg) = parse_error(&html) {
            return Err(err_msg);
        }
        Ok(html)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ConfirmTicketPayload {
        #[serde(rename(serialize = "dummyId"))]
        pub personal_id: String,

        #[serde(rename(serialize = "dummyPhone"))]
        pub phone_num: String,

        #[serde(rename(
            serialize = "TicketMemberSystemInputPanel:TakerMemberSystemDataView:memberSystemRadioGroup"
        ))]
        pub member_radio: String, // 非高鐵會員, 企業會員 / 高鐵會員 / 企業會員統編

        #[serde(rename(serialize = "BookingS3FormSP:hf:0"), default)]
        form_mark: String,

        #[serde(rename(serialize = "idInputRadio"), default)]
        id_input_radio: u8, // 0: 身份證字號 / 1: 護照號碼

        #[serde(rename(serialize = "diffOver"), default = "default_diff_over")]
        diff_over: u8,

        #[serde(rename(serialize = "email"), default)]
        email: String,

        #[serde(rename(serialize = "agree"), default = "default_agree")]
        agree: String,

        #[serde(rename(serialize = "isGoBackM"), default)]
        go_back_m: String,

        #[serde(rename(serialize = "backHome"), default)]
        back_home: String,

        #[serde(rename(serialize = "TgoError"), default)]
        tgo_error: u8,
    }

    fn default_diff_over() -> u8 {
        1
    }

    fn default_agree() -> String {
        "on".to_string()
    }

    impl Default for ConfirmTicketPayload {
        fn default() -> Self {
            ConfirmTicketPayload {
                personal_id: "".to_string(),
                phone_num: "".to_string(),
                member_radio: "0".to_string(),
                form_mark: "".to_string(),
                id_input_radio: 0,
                diff_over: default_diff_over(),
                email: "".to_string(),
                agree: default_agree(),
                go_back_m: "".to_string(),
                back_home: "".to_string(),
                tgo_error: 1,
            }
        }
    }

    impl ConfirmTicketPayload {
        pub fn input_personal_id(&mut self, personal_id: &Option<String>) -> String {
            let input = match personal_id.clone() {
                Some(id) => id,
                None => {
                    println!("Input personal ID (default: 1):");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap_or_default();
                    let input: String = input.trim().to_string();
                    input
                }
            };

            self.personal_id = input.trim().to_string();
            self.personal_id.clone()
        }
    }

    fn process_membership(page: &Html, membership_id: &Option<String>) -> (String, Option<String>) {
        let sel_str = match membership_id {
            Some(_) => "#memberSystemRadio1",
            None => "#memberSystemRadio3",
        };

        let membership_selector = Selector::parse(sel_str).unwrap();
        let elem = page.select(&membership_selector).next().unwrap();
        let membership_radio = elem.attr("value").unwrap();

        if let Some(membership_id) = membership_id {
            let payload = vec![
                (
                    "TicketMemberSystemInputPanel:TakerMemberSystemDataView:memberSystemRadioGroup:memberShipNumber",
                    membership_id.clone(),
                ),
                (
                    "TicketMemberSystemInputPanel:TakerMemberSystemDataView:memberSystemRadioGroup:memberSystemShipCheckBox",
                    "on".to_string(),
                ),
            ];
            let encoded_payload = serde_urlencoded::to_string(&payload).unwrap();
            return (membership_radio.to_string(), Some(encoded_payload));
        }

        (membership_radio.to_string(), None)
    }

    fn process_early_bird(page: &Html, personal_id: &str) -> Option<HashMap<String, String>> {
        let selector = Selector::parse(".superEarlyBird").unwrap();
        let elem: Vec<String> = page
            .select(&selector)
            .filter_map(|tag| tag.text().next().map(|text| text.to_string()))
            .collect();

        if elem.is_empty() {
            return None;
        }

        let early_type_selector = Selector::parse(
            "input[name='TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataTypeName']").unwrap();
        let early_type_elem = page.select(&early_type_selector).next().unwrap();
        let early_type = early_type_elem.attr("value").unwrap().to_string();

        let mut additional_payload = HashMap::from([
            (
                "TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataLastName".to_string(),
                "".to_string(),
            ),
            (
                "TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataFirstName".to_string(),
                "".to_string(),
            ),
            (
                "TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataTypeName".to_string(),
                early_type.clone(),
            ),
            (
                "TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataIdNumber".to_string(),
                personal_id.to_string(),
            ),
            (
                "TicketPassengerInfoInputPanel:passengerDataView:0:passengerDataView2:passengerDataInputChoice".to_string(),
                "0".to_string(), // 0 for ID, 1 for passport
            ),
        ]);

        for i in 1..elem.len() {
            println!(
                "Input passenger info for passenger {}\n(ID change is not allowed after input!):",
                i + 1
            );
            let mut inp_id = String::new();
            std::io::stdin().read_line(&mut inp_id).unwrap_or_default();

            additional_payload.insert(
                format!("TicketPassengerInfoInputPanel:passengerDataView:{i}:passengerDataView2:passengerDataLastName"),
                "".to_string(),
            );
            additional_payload.insert(
                format!("TicketPassengerInfoInputPanel:passengerDataView:{i}:passengerDataView2:passengerDataFirstName"),
                "".to_string(),
            );
            additional_payload.insert(
                format!("TicketPassengerInfoInputPanel:passengerDataView:{i}:passengerDataView2:passengerDataTypeName"),
                early_type.clone(),
            );
            additional_payload.insert(
                format!("TicketPassengerInfoInputPanel:passengerDataView:{i}:passengerDataView2:passengerDataIdNumber"),
                inp_id.trim().to_string(),
            );
            additional_payload.insert(
                format!("TicketPassengerInfoInputPanel:passengerDataView:{i}:passengerDataView2:passengerDataInputChoice"),
                "0".to_string(), // 0 for ID, 1 for passport
            );
        }
        Some(additional_payload)
    }
}

fn show_result(page: &Html) {
    let pnr_code_selector = Selector::parse("p.pnr-code span").unwrap();
    let pnr_code_span_tag = page.select(&pnr_code_selector).next().unwrap();
    let pnr_code = pnr_code_span_tag.text().next().unwrap();

    println!("\nPlease use the following PNR code for payment and picking up the ticket:");
    println!("PNR Code: {}", pnr_code);

    // Price
    let price_selector = Selector::parse("#setTrainTotalPriceValue").unwrap();
    let price_tag = page.select(&price_selector).next().unwrap();
    let price = price_tag.text().next().unwrap();

    let payment_status_selector = Selector::parse("span.status-unpaid span:nth-child(3)").unwrap();
    let payment_status_tag = page.select(&payment_status_selector).next().unwrap();
    let payment_exp_date = payment_status_tag.text().next().unwrap();
    println!("Price: {}. Please pay before {}", price, payment_exp_date);
    println!("-------(Ticket Information)-------");

    // Departure date
    let depart_date_selector = Selector::parse("span.date span").unwrap();
    let depart_date_tag = page.select(&depart_date_selector).next().unwrap();
    let depart_date = depart_date_tag.text().next().unwrap();
    println!("{:>7}{}", "Date: ", depart_date);

    // Departure and arrival time
    let depart_time_selector = Selector::parse("#setTrainDeparture0").unwrap();
    let depart_time_tag = page.select(&depart_time_selector).next().unwrap();
    let depart_time = depart_time_tag.text().next().unwrap();

    let arrive_time_selector = Selector::parse("#setTrainArrival0").unwrap();
    let arrive_time_tag = page.select(&arrive_time_selector).next().unwrap();
    let arrive_time = arrive_time_tag.text().next().unwrap();

    println!(
        "{:>7}{}",
        "Time: ",
        format!("{}~{}", depart_time, arrive_time)
    );

    // Station
    let depart_from_selector = Selector::parse("p.departure-stn span").unwrap();
    let depart_from_tag = page.select(&depart_from_selector).next().unwrap();
    let depart_from = depart_from_tag.text().next().unwrap();
    println!("{:>7}{}", "From: ", depart_from);

    let arrive_to_selector = Selector::parse("p.arrival-stn span").unwrap();
    let arrive_to_tag = page.select(&arrive_to_selector).next().unwrap();
    let arrive_to = arrive_to_tag.text().next().unwrap();
    println!("{:>7}{}", "To: ", arrive_to);

    // Seat info
    let seats_selector = Selector::parse("div.seat-label span").unwrap();
    let seats: Vec<String> = page
        .select(&seats_selector)
        .filter_map(|tag| tag.text().next().map(|text| text.to_string()))
        .collect();

    let passenger_count_selector = Selector::parse("div.uk-accordion-content span").unwrap();
    let passenger_count_tag = page.select(&passenger_count_selector).next().unwrap();
    let passenger_count = passenger_count_tag.text().next().unwrap();

    let seat_type_selector = Selector::parse("p.info-data span").unwrap();
    let seat_type_tag = page.select(&seat_type_selector).next().unwrap();
    let seat_type = seat_type_tag.text().next().unwrap();
    println!("Class: {}{}", seat_type, passenger_count);
    println!("Seats: {}", seats.join(", "));
}
