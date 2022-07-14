//! Implements Node bluetooth mesh interface

use crate::{Result, SessionInner};
use std::sync::Arc;
use std::{
    collections::{HashMap}
};

use dbus::{
    arg::{RefArg, Variant},
    nonblock::{Proxy, SyncConnection},
    Path,
};

use crate::mesh::{PATH, SERVICE_NAME, TIMEOUT};
use crate::{Error, ErrorKind, InternalErrorKind};

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
    pub async fn publish(&self, data: heapless::Vec<u8, 384>) -> Result<()> {
        println!("Publishing");
        let path_value =
            Path::new("/mesh_server/ele00").map_err(|_| Error::new(ErrorKind::Internal(InternalErrorKind::InvalidValue)))?;
        let mut options: HashMap<&'static str, Variant<Box<dyn RefArg>>> = HashMap::new();
        self.call_method("Publish", (path_value, 0x1100 as u16, options, data.to_vec())).await?;
        Ok(())
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, self.path.clone(), TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);
}
