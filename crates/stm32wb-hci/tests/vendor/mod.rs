#![allow(dead_code)]

use std::{ops::DerefMut, sync::Mutex};

struct DataBuffer(Vec<u8>);

impl embedded_io::ErrorType for DataBuffer {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Write for DataBuffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.0.extend_from_slice(buf);

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct RecordingSink {
    data: Mutex<DataBuffer>,
}

impl RecordingSink {
    pub fn new() -> RecordingSink {
        RecordingSink {
            data: Mutex::new(DataBuffer(Vec::new())),
        }
    }

    pub fn written_data(&self) -> Vec<u8> {
        self.data.lock().unwrap().0.clone()
    }
}

impl embedded_io::ErrorType for RecordingSink {
    type Error = embedded_io::ErrorKind;
}

impl bt_hci::controller::Controller for RecordingSink {
    async fn write_acl_data(
        &self,
        _packet: &bt_hci::data::AclPacket<'_>,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn write_iso_data(
        &self,
        _packet: &bt_hci::data::IsoPacket<'_>,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn write_sync_data(
        &self,
        _packet: &bt_hci::data::SyncPacket<'_>,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn read<'a>(
        &self,
        _buf: &'a mut [u8],
    ) -> Result<bt_hci::ControllerToHostPacket<'a>, Self::Error> {
        todo!()
    }
}

impl<C> bt_hci::controller::ControllerCmdSync<C> for RecordingSink
where
    C: bt_hci::cmd::SyncCmd,
{
    async fn exec(&self, cmd: &C) -> Result<C::Return, bt_hci::cmd::Error<Self::Error>> {
        use bt_hci::FromHciBytes;
        use bt_hci::WriteHci;
        use bt_hci::transport::WithIndicator;

        let mut data = self.data.lock().unwrap();

        WithIndicator::new(cmd)
            .write_hci(data.deref_mut())
            .map_err(|_| bt_hci::cmd::Error::Io(embedded_io::ErrorKind::InvalidData))?;

        // Allocate enough zero bytes for any fixed or bounded declarative
        // return used by these wire-focused tests.
        let return_buf = vec![0u8; size_of::<C::Return>()];

        C::Return::from_hci_bytes_complete(&return_buf)
            .map_err(|_| bt_hci::cmd::Error::Io(embedded_io::ErrorKind::InvalidData))
    }
}

impl<C> bt_hci::controller::ControllerCmdAsync<C> for RecordingSink
where
    C: bt_hci::cmd::AsyncCmd,
{
    async fn exec(&self, cmd: &C) -> Result<(), bt_hci::cmd::Error<Self::Error>> {
        use bt_hci::WriteHci;
        use bt_hci::transport::WithIndicator;

        let mut data = self.data.lock().unwrap();

        WithIndicator::new(cmd)
            .write_hci(data.deref_mut())
            .map_err(|_| bt_hci::cmd::Error::Io(embedded_io::ErrorKind::InvalidData))?;

        Ok(())
    }
}
