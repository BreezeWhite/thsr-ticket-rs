pub static STATION_MAP: [&str; 12] = [
    "Nangang", "Taipei", "Banqiao", "Taoyuan", "Hsinchu", "Miaoli", "Taichung", "Changhua",
    "Yunlin", "Chiayi", "Tainan", "Zuouing",
];

pub static TIME_TABLE: [&str; 38] = [
    "1201A", "1230A", "600A", "630A", "700A", "730A", "800A", "830A", "900A", "930A", "1000A",
    "1030A", "1100A", "1130A", "1200N", "1230P", "100P", "130P", "200P", "230P", "300P", "330P",
    "400P", "430P", "500P", "530P", "600P", "630P", "700P", "730P", "800P", "830P", "900P", "930P",
    "1000P", "1030P", "1100P", "1130P",
];

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum TicketType {
    Adult = 70,    // F
    Child = 72,    // H
    Disabled = 87, // W
    Elder = 69,    // E
    College = 80,  // P
}
