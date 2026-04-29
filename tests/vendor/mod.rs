#![allow(dead_code)]

extern crate stm32wb_hci as hci;

pub struct RecordingSink {
    pub written_data: Vec<u8>,
}

impl hci::Controller for RecordingSink {
    async fn controller_write(&mut self, f: impl FnOnce(&mut [u8; 256])) {
        self.written_data.resize(256, 0);

        let len = {
            let buf = self.written_data.as_mut_slice();
            f(buf.try_into().unwrap());
            buf[3] + 4
        };

        self.written_data.resize(len.into(), 0);
    }

    async fn controller_read_into(&mut self, _buf: &mut [u8]) {}
}

impl RecordingSink {
    pub fn new() -> RecordingSink {
        RecordingSink {
            written_data: Vec::new(),
        }
    }
}
