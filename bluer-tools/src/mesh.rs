#![feature(generic_associated_types)]
//! Attach and send/receive BT Mesh messages
//! Example receive
//! [bluer/bluer-tools]$ RUST_LOG=TRACE cargo run --bin mesh -- --token e5571d4f4377707a
//!
//! Example send
//! [burrboard/gateway]$ TOKEN=26ea5cc2f46fd59d app/device.py

use bluer::mesh::{application::Application, *};
use clap::Parser;
use colored_json::to_colored_json_auto;
use drogue_device::drivers::ble::mesh::{
    composition::CompanyIdentifier,
    model::{
        firmware::FirmwareUpdateClient,
        generic::onoff::{GenericOnOffClient, GenericOnOffServer},
        sensor::{PropertyId, SensorClient, SensorConfig, SensorData, SensorDescriptor, SensorServer},
        Message, Model,
    },
    pdu::ParseError,
};
use futures::{pin_mut, StreamExt};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    token: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let session = bluer::Session::new().await?;

    let mesh = session.mesh().await?;

    let (element_control, element_handle) = element_control();

    let _app = Application {
        path: "/example".to_string(),
        elements: vec![
            Element {
                models: vec![Box::new(FromDrogue::new(SensorClient::<SensorModel, 1, 1>::new()))],
                control_handle: None,
            },
            Element { models: vec![Box::new(FromDrogue::new(FirmwareUpdateClient))], control_handle: None },
        ],
    };

    let sim = Application {
        path: "/example".to_string(),
        elements: vec![
            Element {
                models: vec![
                    Box::new(FromDrogue::new(GenericOnOffServer)),
                    Box::new(FromDrogue::new(SensorServer::<SensorModel, 1, 1>::new())),
                    Box::new(FromDrogue::new(VendorModel)),
                ],
                control_handle: Some(element_handle),
            },
            Element {
                models: vec![
                    Box::new(FromDrogue::new(GenericOnOffClient)),
                    Box::new(FromDrogue::new(SensorClient::<SensorModel, 1, 1>::new())),
                ],
                control_handle: None,
            },
        ],
    };

    let _registered = mesh.application(sim).await?;

    mesh.attach("/example", &args.token).await?;

    println!("Echo service ready. Press enter to quit.");
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    pin_mut!(element_control);

    loop {
        tokio::select! {
            _ = lines.next_line() => break,
            evt = element_control.next() => {
                match evt {
                    Some(msg) => {
                        println!("{}", to_colored_json_auto(&data_to_json(&msg.payload.parameters))?);

                        //TODO use model to parse payload

                        // let sensor_get: Option<SensorMessage<'_, SensorConfig, 1, 1>> = SensorServer::parse(msg.payload.opcode, &msg.payload.parameters).map_err(|_| std::fmt::Error)?;
                        // match sensor_get {
                        //     Some(value) => {
                        //         match value {
                        //             SensorMessage::Status(data) => {
                        //                 println!("Received status");
                        //             },
                        //             _ => {
                        //                 println!("Received message");
                        //             }
                        //         }
                        //     }
                        // }
                    },
                    None => break,
                }
            },
        }
    }

    //TODO unregister

    Ok(())
}

#[derive(Clone, Debug)]
pub struct SensorModel;

#[derive(Clone, Debug)]
pub struct Temperature(i8);

impl SensorConfig for SensorModel {
    type Data<'m> = Temperature;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PropertyId(0x4F), 1)];
}

impl SensorData for Temperature {
    fn decode(&mut self, id: PropertyId, params: &[u8]) -> Result<(), ParseError> {
        if id.0 == 0x4F {
            self.0 = params[0] as i8;
            Ok(())
        } else {
            Err(ParseError::InvalidValue)
        }
    }

    fn encode<const N: usize>(
        &self, _: PropertyId, xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.extend_from_slice(&self.0.to_le_bytes()).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

const COMPANY_IDENTIFIER: CompanyIdentifier = CompanyIdentifier(0x05F1);
const COMPANY_MODEL: ModelIdentifier = ModelIdentifier::Vendor(COMPANY_IDENTIFIER, 0x0001);

#[derive(Clone, Debug)]
pub struct VendorModel;

impl Model for VendorModel {
    const IDENTIFIER: ModelIdentifier = COMPANY_MODEL;
    type Message<'m> = VendorMessage;

    fn parse<'m>(_opcode: Opcode, _parameters: &'m [u8]) -> Result<Option<Self::Message<'m>>, ParseError> {
        unimplemented!();
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorMessage {}

impl Message for VendorMessage {
    fn opcode(&self) -> Opcode {
        unimplemented!();
    }

    fn emit_parameters<const N: usize>(
        &self, _xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        unimplemented!();
    }
}

fn data_to_json(data: &[u8]) -> serde_json::Value {
    //assert_eq!(data.len(), 22);

    let temp: f32 = (i16::from_le_bytes([data[0], data[1]]) as f32) / 100.0;
    let brightness: u16 = u16::from_le_bytes([data[2], data[3]]);

    let battery: u8 = data[4];

    let counter_a = u16::from_le_bytes([data[5], data[6]]);
    let counter_b = u16::from_le_bytes([data[7], data[8]]);

    let accel: (f32, f32, f32) = (
        f32::from_le_bytes([data[9], data[10], data[11], data[12]]),
        f32::from_le_bytes([data[13], data[14], data[15], data[16]]),
        f32::from_le_bytes([data[17], data[18], data[19], data[20]]),
    );

    let buttons_leds = data[21];
    let button_a = (buttons_leds & 0x1) != 0;
    let button_b = ((buttons_leds >> 1) & 0x1) != 0;

    let red_led = ((buttons_leds >> 2) & 0x1) != 0;
    let green_led = ((buttons_leds >> 3) & 0x1) != 0;
    let blue_led = ((buttons_leds >> 4) & 0x1) != 0;
    let yellow_led = ((buttons_leds >> 5) & 0x1) != 0;

    json!({"temperature": {"value": temp}, "light": { "value": brightness },
            "led_1": { "state": red_led },
            "led_2": { "state": green_led },
            "led_3": { "state": blue_led },
            "led_4": { "state": yellow_led },
            "accelerometer": {
        "x": accel.0,
        "y": accel.1,
        "z": accel.2,
            }, "device": { "battery": (battery as f32) / 100.0 }, "button_a": { "presses": counter_a, "state": button_a  } , "button_b": { "presses": counter_b, "state": button_b} })
}
