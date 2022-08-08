#![feature(generic_associated_types)]
//! Attach and send/receive BT Mesh messages
//!
//! Example meshd
//! [burrboard/gateway]$ sudo /usr/libexec/bluetooth/bluetooth-meshd --config ${PWD}/deploy/bluez/example/meshcfg --storage ${PWD}/deploy/bluez/example/mesh --debug
//!
//! Example receive
//! [bluer]$ RUST_LOG=TRACE cargo run --example mesh_sensor_client -- --token 7eb48c91911361da
//!
//! Example send
//! [burrboard/gateway]$ TOKEN=dae519a06e504bd3 ./app/temp-device.py

use bluer::{mesh::{application::Application, *, provisioner::{ProvisionerControlHandle, Provisioner}}, Uuid};
use clap::Parser;
use dbus::Path;
use drogue_device::drivers::ble::mesh::{
    model::{
        foundation::configuration::{ConfigurationServer, ConfigurationClient},
    },
};
use futures::{StreamExt};
use tokio::{signal, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;
use std::sync::Arc;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    token: String,
    #[clap(short, long)]
    uuid: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let session = bluer::Session::new().await?;

    let mesh = session.mesh().await?;

    let (_, element_handle) = element_control();

    let root_path = Path::from("/mesh/cfgclient");
    let app_path = Path::from(format!("{}/{}", root_path.clone(), "application"));
    let element_path = Path::from(format!("{}/{}", root_path.clone(), "ele00"));

    let (prov_tx, prov_rx) = mpsc::channel(1);

    let sim = Application {
        path: app_path,
        elements: vec![Element {
            path: element_path,
            models: vec![
                Arc::new(FromDrogue::new(ConfigurationServer::default())),
                Arc::new(FromDrogue::new(ConfigurationClient::default())),
            ],
            control_handle: Some(element_handle),
        }],
        provisioner: Some(Provisioner {
            control_handle: ProvisionerControlHandle {
                messages_tx: prov_tx,
            }
        }),
    };

    let _registered = mesh.application(root_path.clone(), sim).await?;

    let node = mesh.attach(root_path.clone(), &args.token).await?;

    if let Some(management) = node.management {
        management.add_node(Uuid::parse_str(&args.uuid)?).await?;
    }

    let mut prov_stream = ReceiverStream::new(prov_rx);

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            evt = prov_stream.next() => {
                match evt {
                    Some(msg) => {
                        println!("msg {:?}", msg);
                    },
                    None => break,
                }
            },
        }
    }

    //TODO unregister

    Ok(())
}
