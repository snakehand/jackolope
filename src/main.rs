use std::time::Duration;

use serialport::SerialPort;

mod protocol;

use protocol::*;

fn get_response(port: &mut Box<dyn SerialPort>) -> Result<Response, Box<dyn std::error::Error>> {
    let mut buffer = [0; 1];
    loop {
        port.read(&mut buffer)?;
        if buffer[0] & 0x80 == 0 {
            continue;
        }
        let resp_type = buffer[0] & 0x7F;
        port.read(&mut buffer)?;
        if buffer[0] & 0x80 != 0 {
            continue;
        }
        let mut length = (buffer[0] as usize) << 7;
        port.read(&mut buffer)?;
        if buffer[0] & 0x80 != 0 {
            continue;
        }
        length |= buffer[0] as usize;
        if length < 3 {
            return Err("Invalid response length".into());
        }
        length -= 3;
        println!("Response type: {}, length: {}", resp_type, length);
        let mut data = Vec::with_capacity(length);
        for _ in 0..length {
            port.read(&mut buffer)?;
            data.push(buffer[0]);
        }
        if let Some(rtype) = MessageType::try_from_byte(resp_type) {
            let response = match Response::try_from_raw(rtype, &data) {
                Ok(r) => r,
                Err(e) => {
                    println!(
                        "Received response: {:?}({}) but failed to parse: {:?}",
                        rtype, resp_type, e
                    );
                    return Err("Parse error".into());
                }
            };
            return Ok(response);
        } else {
            println!("Received response: Unknown({})", resp_type);
            return Err("Invalid response type".into());
        }
    }
}

fn main() {
    println!("Hello, world!");

    let port_name = "/dev/tty.usbserial-1120";

    let mut port = serialport::new(port_name, 9600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::Hardware)
        .timeout(Duration::from_millis(1000))
        .open()
        .unwrap();

    port.write(&Command::Reset.as_byte()).unwrap(); // Reset the device
    port.write(&Command::RequestBoard.as_byte()).unwrap(); // Reset the device
    println!("{:?}", get_response(&mut port).unwrap());
    port.write(&Command::RequestSerialNumber.as_byte()).unwrap(); // Reset the device
    println!("{:?}", get_response(&mut port).unwrap());

    port.write(&Command::RequestUpdate.as_byte()).unwrap(); // Reset the device

    loop {
        match get_response(&mut port) {
            Ok(response) => {
                println!("Received response: {:?}", response);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}
