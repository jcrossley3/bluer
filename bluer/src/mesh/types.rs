use core::fmt::Debug;
use drogue_device::drivers::ble::mesh::model::Model as ConcreteModel;
pub use drogue_device::drivers::ble::mesh::{
    model::{Message as ConcreteMessage, ModelIdentifier},
    pdu::{access::Opcode, ParseError},
    InsufficientBuffer,
};

pub trait Message {
    fn opcode(&self) -> Opcode;
    fn emit_parameters(&self, xmit: &mut Vec<u8>);
}

pub trait Model: Sync + Send + Debug {
    fn identifier(&self) -> ModelIdentifier;
    fn supports_subscription(&self) -> bool;
    fn supports_publication(&self) -> bool;

    fn parse<'m>(opcode: Opcode, parameters: &'m [u8]) -> Result<Option<Box<dyn Message + 'm>>, ParseError>
    where
        Self: Sized + 'm;
}

pub struct ModelMessage<M> {
    m: M,
}

impl<M> Message for ModelMessage<M>
where
    M: ConcreteMessage,
{
    fn opcode(&self) -> Opcode {
        self.m.opcode()
    }

    fn emit_parameters(&self, xmit: &mut Vec<u8>) {
        let mut v: heapless::Vec<u8, 512> = heapless::Vec::new();
        self.m.emit_parameters(&mut v).unwrap();
        xmit.extend_from_slice(&v[..]);
    }
}

#[derive(Debug)]
pub struct FromDrogue<M>
where
    M: Debug,
{
    _m: core::marker::PhantomData<M>,
}

impl<M: Debug> FromDrogue<M> {
    pub fn new(m: M) -> Self {
        Self { _m: core::marker::PhantomData }
    }
}

unsafe impl<M> Sync for FromDrogue<M> where M: Debug {}
unsafe impl<M> Send for FromDrogue<M> where M: Debug {}

impl<M> Model for FromDrogue<M>
where
    M: ConcreteModel + Debug,
{
    fn identifier(&self) -> ModelIdentifier {
        M::IDENTIFIER
    }
    fn supports_subscription(&self) -> bool {
        M::SUPPORTS_SUBSCRIPTION
    }

    fn supports_publication(&self) -> bool {
        M::SUPPORTS_PUBLICATION
    }

    fn parse<'m>(opcode: Opcode, parameters: &'m [u8]) -> Result<Option<Box<dyn Message + 'm>>, ParseError>
    where
        Self: 'm,
    {
        let m = M::parse(opcode, parameters)?;
        if let Some(m) = m {
            let b: Box<dyn Message + 'm> = Box::new(ModelMessage { m });
            Ok(Some(b))
        } else {
            Ok(None)
        }
    }
}
