//! Bluetooth Mesh module

pub mod application;
pub mod network;
mod types;
pub use types::*;

use crate::{method_call, Result, SessionInner, Error, ErrorKind, ERR_PREFIX};
use dbus::{
    arg::{PropMap, RefArg, Variant},
    nonblock::{stdintf::org_freedesktop_dbus::ObjectManager, Proxy, SyncConnection},
    Path,
};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};
use drogue_device::drivers::ble::mesh::{
    address::{Address, UnicastAddress},
    app::ApplicationKeyIdentifier,
    pdu::{
        access::{AccessPayload},
    },
};
use std::{
    collections::HashMap,
    fmt,
    num::NonZeroU16,
    pin::Pin,
    sync::{Arc},
    task::Poll,
    time::Duration,
};
use pin_project::pin_project;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use futures::{Stream};
use strum::{IntoStaticStr};

pub(crate) const SERVICE_NAME: &str = "org.bluez.mesh";
pub(crate) const PATH: &str = "/org/bluez/mesh";
pub(crate) const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) const ELEMENT_INTERFACE: &str = "org.bluez.mesh.Element1";

/// Gets all D-Bus objects from the BlueZ service.
async fn all_dbus_objects(
    connection: &SyncConnection,
) -> Result<HashMap<Path<'static>, HashMap<String, PropMap>>> {
    let p = Proxy::new(SERVICE_NAME, "/", TIMEOUT, connection);
    Ok(p.get_managed_objects().await?)
}

type ElementConfig = HashMap<String, Variant<Box<dyn RefArg + 'static>>>;

/// Interface to a Bluetooth mesh element interface.
#[derive(Debug)]
pub struct Element {
    /// Element models
    pub models: Vec<Box<dyn Model>>,
    /// Control handle for element once it has been registered.
    pub control_handle: Option<ElementControlHandle>,
}

/// An element exposed over D-Bus to bluez.
pub struct RegisteredElement {
    inner: Arc<SessionInner>,
    element: Element,
    index: u8,
}

impl RegisteredElement {
    pub(crate) fn new(inner: Arc<SessionInner>, element: Element, index: u8) -> Self {
        Self { inner, element, index }
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, PATH, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(ELEMENT_INTERFACE);

    pub(crate) fn register_interface(cr: &mut Crossroads) -> IfaceToken<Arc<Self>> {
        cr.register(ELEMENT_INTERFACE, |ib: &mut IfaceBuilder<Arc<Self>>| {
            ib.method_with_cr_async(
                "MessageReceived",
                ("source", "key_index", "destination", "data"),
                (),
                |ctx,
                 cr,
                 (source, key_index, destination, data): (
                    u16,
                    u16,
                    Variant<Box<dyn RefArg + 'static>>,
                    Vec<u8>,
                )| {
                    method_call(ctx, cr, move |reg: Arc<Self>| async move {
                        log::trace!(
                            "Message received for element {:?}: (source: {:?}, key_index: {:?}, dest: {:?}, data: {:?})",
                            reg.index,
                            source,
                            key_index,
                            destination,
                            data
                        );

                        let key = ApplicationKeyIdentifier::from(u8::try_from(key_index).unwrap_or_default());
                        let src: UnicastAddress = source.try_into().map_err(|_| ReqError::Failed)?;
                        // TODO handle virtual addresses
                        let value = &destination.0;
                        let dest = Address::parse(dbus::arg::cast::<u16>(value).unwrap().to_be_bytes());
                        let payload = AccessPayload::parse(&data).map_err(|_| ReqError::Failed)?;

                        let msg = ElementMessage {
                            key, src, dest, payload
                        };

                        match &reg.element.control_handle {
                            Some(handler) => {
                                handler
                                .messages_tx
                                .send(msg)
                                .await
                                .map_err(|_| ReqError::Failed)?;
                            }
                            None => ()
                        }

                        Ok(())
                    })
                },
            );
            cr_property!(ib, "Index", reg => {
                Some(reg.index)
            });
            cr_property!(ib, "Models", reg => {
                let mut mt: Vec<(u16, ElementConfig)> = vec![];
                // TODO rewrite
                for model in &reg.element.models {
                    if let ModelIdentifier::SIG(id) = model.identifier() {
                        // TODO what about opts?
                        mt.push((id, HashMap::new()));
                    }
                }
                Some(mt)
            });
            cr_property!(ib, "VendorModels", reg => {
                let mut mt: Vec<(u16, u16, ElementConfig)> = vec![];
                for model in &reg.element.models {
                    if let ModelIdentifier::Vendor(vid, id) = model.identifier() {
                        mt.push((vid.0, id, HashMap::new()));
                    }
                }
                Some(mt)
            });
        })
    }
}

/// An object to control a element and receive messages once it has been registered.
///
/// Use [element_control] to obtain controller and associated handle.
#[pin_project]
pub struct ElementControl {
    handle_rx: watch::Receiver<Option<NonZeroU16>>,
    #[pin]
    messages_rx: ReceiverStream<ElementMessage>,
}

impl fmt::Debug for ElementControl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ElementControl {{ handle: {} }}", self.handle().map(|h| h.get()).unwrap_or_default())
    }
}

impl ElementControl {
    /// Gets the assigned handle of the element.
    pub fn handle(&self) -> crate::Result<NonZeroU16> {
        match *self.handle_rx.borrow() {
            Some(handle) => Ok(handle),
            None => Err(Error::new(ErrorKind::NotRegistered)),
        }
    }
}

impl Stream for ElementControl {
    type Item = ElementMessage;

    fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context) -> Poll<Option<Self::Item>> {
        self.project().messages_rx.poll_next(cx)
    }
}

/// A handle to store inside a element definition to make it controllable
/// once it has been registered.
///
/// Use [element_control] to obtain controller and associated handle.
pub struct ElementControlHandle {
    handle_tx: watch::Sender<Option<NonZeroU16>>,
    messages_tx: mpsc::Sender<ElementMessage>,
}

impl Default for ElementControlHandle {
    fn default() -> Self {
        Self { handle_tx: watch::channel(None).0, messages_tx: mpsc::channel(1).0 }
    }
}

impl fmt::Debug for ElementControlHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ElementControlHandle")
    }
}

/// Creates a [ElementControl] and its associated [ElementControlHandle].
///
/// Keep the [ElementControl] and store the [ElementControlHandle] in [Element::control_handle].
pub fn element_control() -> (ElementControl, ElementControlHandle) {
    let (handle_tx, handle_rx) = watch::channel(None);
    let (messages_tx, messages_rx) = mpsc::channel(1);
    (
        ElementControl { handle_rx, messages_rx: ReceiverStream::new(messages_rx) },
        ElementControlHandle { handle_tx, messages_tx },
    )
}

// ===========================================================================================
// Request error
// ===========================================================================================

/// Error response from us to a Bluetooth request.
#[derive(Clone, Copy, Debug, displaydoc::Display, Eq, PartialEq, Ord, PartialOrd, Hash, IntoStaticStr)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ReqError {
    /// Bluetooth request failed
    Failed,
    /// Bluetooth request already in progress
    InProgress,
    /// Invalid offset for Bluetooth GATT property
    InvalidOffset,
    /// Invalid value length for Bluetooth GATT property
    InvalidValueLength,
    /// Bluetooth request not permitted
    NotPermitted,
    /// Bluetooth request not authorized
    NotAuthorized,
    /// Bluetooth request not supported
    NotSupported,
}

impl std::error::Error for ReqError {}

impl Default for ReqError {
    fn default() -> Self {
        Self::Failed
    }
}

impl From<ReqError> for dbus::MethodErr {
    fn from(err: ReqError) -> Self {
        let name: &'static str = err.into();
        Self::from((ERR_PREFIX.to_string() + name, &err.to_string()))
    }
}

/// Result of a Bluetooth request to us.
pub type ReqResult<T> = std::result::Result<T, ReqError>;
