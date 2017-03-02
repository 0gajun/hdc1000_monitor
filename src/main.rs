#![feature(associated_consts)]

extern crate libc;
extern crate hyper;
extern crate chrono;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::{thread, time};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::convert::From;
use std::process;
use libc::c_ulong;

const INFLUX_DB_NAME: &'static str = "myroom";
const INFLUX_DB_ENDPOINT: &'static str = "localhost:8086";

const DURATION: u64 = 60;

struct I2C;

/// Ported from `linux/i2c-dev.h`
impl I2C {
    const I2C_SLAVE: u16 = 0x0703;
}

struct HDC1000;

impl HDC1000 {
    const I2C_ADDR: u8 = 0x40;

    const REGP_TEMP: u8 = 0x00;
    // const REGP_HUMID: u8 = 0x01;
    const REGP_CONFIG: u8 = 0x02;

    const CONF_MODE_AT_ONCE: u16 = 1 << 12;
}

type Humidity = f32;
type Temperature = f32;

fn main() {
    env_logger::init().unwrap();

    let duration = time::Duration::from_secs(DURATION);

    loop {
        if let Err(e) = get_sensor_val().and_then(|(temp, humid)| send_influx_db(&temp, &humid)) {
            error!("{:?}", e);
            process::exit(1);

        }
        thread::sleep(duration);
    }
}

fn get_sensor_val() -> Result<(Temperature, Humidity), CheckError> {
    let mut file = OpenOptions::new().read(true)
        .write(true)
        .open("/dev/i2c-1")
        .or(Err(CheckError::OpenDevice))?;

    // Unsafe due to calling libc directly
    unsafe {
        let raw_fd = file.as_raw_fd();
        if libc::ioctl(raw_fd,
                       I2C::I2C_SLAVE as c_ulong,
                       HDC1000::I2C_ADDR as c_ulong) < 0 {
            return Err(CheckError::Ioctl);
        }
    }

    // Configuration
    let config = HDC1000::CONF_MODE_AT_ONCE;
    let set_value = [HDC1000::REGP_CONFIG, (config >> 8) as u8, (config & 0xff) as u8];
    file.write(&set_value).or(Err(CheckError::Setup))?;

    // Request conversion
    let set_value = [HDC1000::REGP_TEMP];
    file.write(&set_value).or(Err(CheckError::RequestConversion))?;

    // Wait finishing conversion
    let wait_time = time::Duration::from_millis(13); // Should wait 12.85ms
    thread::sleep(wait_time);

    // Retrieve result
    let mut response: [u8; 4] = [0; 4];
    file.read(&mut response).or(Err(CheckError::ReadResult))?;

    // Calculate actual value
    let raw_temp = (response[0] as u16) << 8 | (response[1] as u16);
    let raw_humid = (response[2] as u16) << 8 | (response[3] as u16);
    let temperature = (f32::from(raw_temp) / 65536.0 * 165.0) - 40.0;
    let humidity = f32::from(raw_humid) / 65536.0 * 100.0;

    debug!("[{}] TEMP: {} C, HUMID: {} %",
           chrono::prelude::Local::now().format("%Y-%m-%d %H:%M:%S"),
           temperature,
           humidity);

    Ok((temperature, humidity))
}

fn send_influx_db(temperature: &Temperature, humidity: &Humidity) -> Result<(), CheckError> {
    let timestamp = get_current_timestamp();
    let body_temperature = build_query("temperature", temperature, timestamp);
    let body_humidity = build_query("humidity", humidity, timestamp);

    let client = hyper::Client::new();
    let endpoint = format!("http://{}/write?db={}", INFLUX_DB_ENDPOINT, INFLUX_DB_NAME);

    // Post temperature
    client.post(&endpoint)
        .body(&body_temperature)
        .send()
        .or(Err(CheckError::SendInfluxDB))?;

    // Post humidity
    client.post(&endpoint)
        .body(&body_humidity)
        .send()
        .or(Err(CheckError::SendInfluxDB))?;

    Ok(())
}

fn get_current_timestamp() -> i64 {
    // convert from seconds to nanoseconds
    chrono::prelude::Local::now().timestamp() * 1000 * 1000 * 1000
}

fn build_query<'a>(series_name: &'a str, value: &'a f32, timestamp: i64) -> String {
    format!("{series},sensor=hdc1000 value={value} {timestamp}",
            series = series_name,
            value = value,
            timestamp = timestamp)
}

#[derive(Debug)]
enum CheckError {
    OpenDevice,
    Ioctl,
    Setup,
    RequestConversion,
    ReadResult,
    SendInfluxDB,
}
