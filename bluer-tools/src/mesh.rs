#![feature(generic_associated_types)]
//! Attach and send/receive BT Mesh messages
//! Example receive
//! [bluer/bluer-tools]$ RUST_LOG=TRACE cargo run --bin mesh -- --token e5571d4f4377707a
//!
//! Example send
//! [burrboard/gateway]$ TOKEN=26ea5cc2f46fd59d app/device.py

use bluer::mesh::{application::Application, *};
use clap::Parser;
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
                        println!("Received element message {:?}", msg);
                    },
                    None => break,
                }
            },
        }
    }

    // TODO unregister

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
