//! Implements Node bluetooth mesh interface

use crate::{Result, SessionInner};
use std::sync::Arc;

use dbus::{
    nonblock::{Proxy, SyncConnection},
    Path,
};

use crate::mesh::{PATH, SERVICE_NAME, TIMEOUT};

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
    pub async fn publish(&self) -> Result<()> {
        println!("Publishing");
        Ok(())
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, PATH, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);
}
