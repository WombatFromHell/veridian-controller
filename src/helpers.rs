use chrono::prelude::*;

pub fn get_cur_time() -> String {
    let dt: DateTime<Local> = Local::now();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}
