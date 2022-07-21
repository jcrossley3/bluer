//! Implements Node bluetooth mesh interface

use crate::{Result, SessionInner};
use std::{collections::HashMap, sync::Arc};

use dbus::{
    arg::{RefArg, Variant},
    nonblock::{Proxy, SyncConnection},
    Path,
};

use crate::{
    mesh::{SERVICE_NAME, TIMEOUT},
    Error, ErrorKind,
};
use drogue_device::drivers::ble::mesh::model::{Message, Model, ModelIdentifier};

pub(crate) const INTERFACE: &str = "org.bluez.mesh.Node1";

/// Interface to a Bluetooth mesh node.
pub struct Node {
    inner: Arc<SessionInner>,
    path: Path<'static>,
}

impl Node {
    pub(crate) async fn new(path: Path<'static>, inner: Arc<SessionInner>) -> Result<Self> {
        Ok(Self { inner, path })
    }

    /// Publish message to the mesh
    pub async fn publish<'m, M: Model>(&self, message: M::Message<'m>, path: Path<'m>) -> Result<()> {
        let model_id = match M::IDENTIFIER {
            ModelIdentifier::SIG(id) => id,
            ModelIdentifier::Vendor(_, id) => id,
        };

        let mut data: heapless::Vec<u8, 384> = heapless::Vec::new();
        message.opcode().emit(&mut data).map_err(|_| Error::new(ErrorKind::Failed))?;
        message.emit_parameters(&mut data).map_err(|_| Error::new(ErrorKind::Failed))?;

        let options: HashMap<&'static str, Variant<Box<dyn RefArg>>> = HashMap::new();

        log::trace!("Publishing message: {:?} {:?} {:?} {:?}", path, model_id, options, data.to_vec());
        self.call_method("Publish", (path, model_id, options, data.to_vec())).await?;

        Ok(())
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, self.path.clone(), TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);
}
