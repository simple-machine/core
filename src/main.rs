use smov::{communicate, serialport::SerialPortType};
use std::io::BufRead;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "Simple Machine's OpenCV Ventilator")]
enum Command {
    Detect {
        /// The images to analyse
        #[structopt(parse(from_os_str))]
        files: Vec<PathBuf>,
    },
    Control {
        /// The serial device to which is connected the arduino
        #[structopt(parse(from_os_str))]
        serial: Option<PathBuf>,
    },
}

fn main() {
    match Command::from_args() {
        Command::Detect { files } => unimplemented!(),
        Command::Control { serial } => {
            let file = if let Some(s) = serial {
                s
            } else {
                match serialport::available_ports() {
                    Ok(v) => {
                        if let Some(s) = v
                            .iter()
                            .find(|s| matches!(s.port_type, SerialPortType::UsbPort(..)))
                        {
                            println!("Using port {}", s.port_name);
                            s.port_name.as_str().into()
                        } else {
                            eprintln!("No serial port available");
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Could not list serial devices: {}", e);
                        std::process::exit(1);
                    }
                }
            };
            match communicate(file) {
                Ok((tx, handle)) => {
                    let stdin = std::io::stdin();
                    for line in stdin.lock().lines() {
                        let speed = line.unwrap().trim().parse::<i16>().unwrap();
                        if tx.send(speed).is_err() {
                            break;
                        }
                    }
                    if let Err(e) = handle.join().unwrap() {
                        eprintln!("Fatal error during communication: {}", e);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Could not start communication: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
