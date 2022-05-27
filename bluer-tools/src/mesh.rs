#![feature(generic_associated_types)]
//! Join a BLE mesh

// use uuid::Uuid;
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
};
use futures::future;

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

    let _app = Application {
        path: "/example".to_string(),
        elements: vec![
            Element { models: vec![Box::new(FromDrogue::new(SensorClient::<SensorModel, 1, 1>::new()))] },
            Element { models: vec![Box::new(FromDrogue::new(FirmwareUpdateClient))] },
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
            },
            Element {
                models: vec![
                    Box::new(FromDrogue::new(GenericOnOffClient)),
                    Box::new(FromDrogue::new(SensorClient::<SensorModel, 1, 1>::new())),
                ],
            },
        ],
    };

    let _registered = mesh.application(sim).await?;

    mesh.print_dbus_objects().await?;

    //mesh.join("/example", Uuid::new_v4()).await?;

    mesh.attach("/example", &args.token).await?;

    //mesh.cancel().await?;

    //mesh.leave(token).await?;

    future::pending::<()>().await;

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
