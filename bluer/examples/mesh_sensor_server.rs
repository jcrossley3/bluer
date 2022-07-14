#![feature(generic_associated_types)]
//! Attach and send/receive BT Mesh messages
//!
//! Example meshd
//! [burrboard/gateway]$ sudo /usr/libexec/bluetooth/bluetooth-meshd --config ${PWD}/deploy/bluez/example/meshcfg --storage ${PWD}/deploy/bluez/example/mesh --debug
//!
//! Example receive
//! [bluer/bluer-tools]$ RUST_LOG=TRACE cargo run --example mesh_sensor_server -- --token 7eb48c91911361da
//!
//! Example send
//! [burrboard/gateway]$ TOKEN=dae519a06e504bd3 ./app/temp-device.py

use bluer::mesh::{application::Application, *};
use clap::Parser;
use drogue_device::drivers::ble::mesh::{
    composition::CompanyIdentifier,
    model::{
        sensor::{PropertyId, SensorConfig, SensorData, SensorDescriptor, SensorServer, SensorMessage, SensorStatus},
        Message, Model,
    },
    pdu::ParseError,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    signal,
};
use bluer::{Error, ErrorKind};

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

    let (_element_control, element_handle) = element_control();

    let path = "/mesh_server";

    let sim = Application {
        path: path.to_string(),
        elements: vec![Element {
            models: vec![Box::new(FromDrogue::new(SensorServer::<SensorModel, 1, 1>::new()))],
            control_handle: Some(element_handle),
        }],
    };

    let _registered = mesh.application(sim).await?;

    let node = mesh.attach(path, &args.token).await?;

    println!("Snesor server ready. Press enter to send a message. Press Ctrl+C to quit");
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    loop {
        tokio::select! {
            _ = lines.next_line() => {
                println!("Send");
                let message: SensorMessage<'_, SensorModel, 1, 1> = SensorMessage::Status(SensorStatus::new(Temperature(21.0)));
                let mut parameters: heapless::Vec<u8, 384> = heapless::Vec::new();
                message.opcode().emit(&mut parameters).map_err(|_|  Error::new(ErrorKind::Failed))?;
                message.emit_parameters(&mut parameters).map_err(|_|  Error::new(ErrorKind::Failed))?;
                println!("Data {:?}", parameters);

                node.publish(parameters).await?;
            },
            _ = signal::ctrl_c() => break,
        }
    }

    println!("Shutting down");
    //TODO unregister

    Ok(())
}

#[derive(Clone, Debug)]
pub struct SensorModel;

#[derive(Clone, Debug, Default)]
pub struct Temperature(f32);

impl SensorConfig for SensorModel {
    type Data<'m> = Temperature;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PropertyId(0x4F), 1)];
}

impl SensorData for Temperature {
    fn decode(&mut self, id: PropertyId, params: &[u8]) -> Result<(), ParseError> {
        if id.0 == 0x4F {
            self.0 = params[0] as f32 / 2.0;
            Ok(())
        } else {
            Err(ParseError::InvalidValue)
        }
    }

    fn encode<const N: usize>(
        &self, _: PropertyId, xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let value = (self.0 * 2 as f32) as u8;
        xmit.extend_from_slice(&value.to_le_bytes()).map_err(|_| InsufficientBuffer)?;
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
