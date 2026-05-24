use aligned::{A4, Aligned};
use bt_hci::{WriteHci, transport::WithIndicator};
use core::{
    cell::RefCell,
    future::poll_fn,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering, compiler_fence},
    task::Poll,
};
use defmt::{debug, trace};
use embassy_stm32::{
    Peri, interrupt,
    ipcc::{
        Config as IpccConfig, Ipcc, IpccRxChannel, IpccTxChannel, ReceiveInterruptHandler,
        TransmitInterruptHandler,
    },
    peripherals::IPCC,
};
use embassy_sync::{
    blocking_mutex::{self, raw::NoopRawMutex},
    mutex::Mutex,
    signal::Signal,
    waitqueue::AtomicWaker,
};

const TL_PACKET_HEADER_SIZE: usize = mem::size_of::<LinkedListNode>();
const TL_EVT_HEADER_SIZE: usize = 3;
const TL_CS_EVT_SIZE: usize = mem::size_of::<CsEvt>();
const CFG_TL_BLE_EVT_QUEUE_LENGTH: usize = 5;
const CFG_TL_BLE_MOST_EVENT_PAYLOAD_SIZE: usize = 255;
const TL_BLE_EVENT_FRAME_SIZE: usize = TL_EVT_HEADER_SIZE + CFG_TL_BLE_MOST_EVENT_PAYLOAD_SIZE;
const POOL_SIZE: usize =
    CFG_TL_BLE_EVT_QUEUE_LENGTH * 4 * (TL_PACKET_HEADER_SIZE + TL_BLE_EVENT_FRAME_SIZE).div_ceil(4);
const TL_BLEEVT_CC_OPCODE: u8 = 0x0E;
const TL_BLEEVT_CS_OPCODE: u8 = 0x0F;
const SHCI_OGF: u16 = 0x3F;

const fn shci_opcode(ocf: u16) -> u16 {
    (SHCI_OGF << 10) + ocf
}

pub fn init(
    ipcc: Peri<'static, IPCC>,
    irqs: impl interrupt::typelevel::Binding<interrupt::typelevel::IPCC_C1_RX, ReceiveInterruptHandler>
    + interrupt::typelevel::Binding<interrupt::typelevel::IPCC_C1_TX, TransmitInterruptHandler>
    + 'static,
    config: IpccConfig,
) -> (Sys<'static>, Ble<'static>, MemoryManager<'static>) {
    // CPU2 discovers all mailbox buffers through this reference table in shared SRAM.
    unsafe {
        TL_REF_TABLE.as_mut_ptr().write_volatile(RefTable {
            device_info_table: TL_DEVICE_INFO_TABLE.as_ptr(),
            ble_table: TL_BLE_TABLE.as_ptr(),
            thread_table: TL_THREAD_TABLE.as_ptr(),
            sys_table: TL_SYS_TABLE.as_ptr(),
            mem_manager_table: TL_MEM_MANAGER_TABLE.as_ptr(),
            traces_table: TL_TRACES_TABLE.as_ptr(),
            mac_802_15_4_table: TL_MAC_802_15_4_TABLE.as_ptr(),
            zigbee_table: TL_ZIGBEE_TABLE.as_ptr(),
            lld_tests_table: TL_LLD_TESTS_TABLE.as_ptr(),
            ble_lld_table: TL_BLE_LLD_TABLE.as_ptr(),
        });

        zero(&mut TL_DEVICE_INFO_TABLE);
        zero(&mut TL_BLE_TABLE);
        zero(&mut TL_THREAD_TABLE);
        zero(&mut TL_SYS_TABLE);
        zero(&mut TL_MEM_MANAGER_TABLE);
        zero(&mut TL_TRACES_TABLE);
        zero(&mut TL_MAC_802_15_4_TABLE);
        zero(&mut TL_ZIGBEE_TABLE);
        zero(&mut TL_LLD_TESTS_TABLE);
        zero(&mut TL_BLE_LLD_TABLE);
        zero(&mut EVT_POOL);
        zero(&mut SYS_SPARE_EVT_BUF);
        zero(&mut CS_BUFFER);
        zero(&mut BLE_SPARE_EVT_BUF);
        zero(&mut BLE_CMD_BUFFER);
        zero(&mut HCI_ACL_DATA_BUFFER);
    }

    compiler_fence(Ordering::SeqCst);

    let [
        (ble_cmd_tx, ble_evt_rx),
        (sys_cmd_tx, sys_evt_rx),
        (_mac_cmd_tx, _mac_evt_rx),
        (mm_release_tx, _traces_rx),
        (_ble_lld_tx, _ble_lld_rx),
        (_, _),
    ] = Ipcc::new(ipcc, irqs, config).split();

    (
        Sys::new(sys_cmd_tx, sys_evt_rx),
        Ble::new(ble_cmd_tx, ble_evt_rx),
        MemoryManager::new(mm_release_tx),
    )
}

unsafe fn zero<T>(slot: &mut Aligned<A4, MaybeUninit<T>>) {
    slot.as_mut_ptr()
        .write_volatile(MaybeUninit::zeroed().assume_init())
}

pub struct Sys<'a> {
    cmd: IpccTxChannel<'a>,
    evt: IpccRxChannel<'a>,
}

impl<'a> Sys<'a> {
    fn new(cmd: IpccTxChannel<'a>, evt: IpccRxChannel<'a>) -> Self {
        unsafe {
            LinkedListNode::init_head(SYSTEM_EVT_QUEUE.as_mut_ptr());
            TL_SYS_TABLE.as_mut_ptr().write_volatile(SysTable {
                pcmd_buffer: SYS_CMD_BUF.as_mut_ptr(),
                sys_queue: SYSTEM_EVT_QUEUE.as_ptr(),
            });
        }

        Self { cmd, evt }
    }

    pub async fn read_ready(&mut self) -> Result<SysEventReady, ()> {
        self.read().await.payload()[0].try_into()
    }

    pub async fn ble_init(&mut self, param: BleInitParam) -> Result<SysCommandStatus, ()> {
        self.write(SHCI_OPCODE_BLE_INIT, param.payload()).await;
        // System command responses are written back into SYS_CMD_BUF after channel clears.
        self.cmd.flush().await;
        unsafe { SysCommandStatus::from_cmd_buffer(SYS_CMD_BUF.as_ptr()) }
    }

    async fn write(&mut self, opcode: u16, payload: &[u8]) {
        self.cmd
            .send(|| unsafe {
                CmdPacket::write_into(
                    SYS_CMD_BUF.as_mut_ptr(),
                    TlPacketType::SysCmd,
                    opcode,
                    payload,
                );
            })
            .await;
    }

    async fn read(&mut self) -> EvtBox<MemoryManager<'_>> {
        self.evt
            .receive(|| unsafe {
                critical_section::with(|cs| {
                    LinkedListNode::remove_head(cs, SYSTEM_EVT_QUEUE.as_mut_ptr())
                        .map(|node| EvtBox::new(node.cast()))
                })
            })
            .await
    }
}

pub struct Ble<'a> {
    cmd: IpccTxChannel<'a>,
    evt: IpccRxChannel<'a>,
}

impl<'a> Ble<'a> {
    fn new(cmd: IpccTxChannel<'a>, evt: IpccRxChannel<'a>) -> Self {
        unsafe {
            LinkedListNode::init_head(EVT_QUEUE.as_mut_ptr());
            TL_BLE_TABLE.as_mut_ptr().write_volatile(BleTable {
                pcmd_buffer: BLE_CMD_BUFFER.as_mut_ptr().cast(),
                pcs_buffer: CS_BUFFER.as_ptr().cast(),
                pevt_queue: EVT_QUEUE.as_ptr().cast(),
                phci_acl_data_buffer: HCI_ACL_DATA_BUFFER.as_mut_ptr().cast(),
            });
        }

        Self { cmd, evt }
    }
}

impl<'a> EventMemoryManager for Ble<'a> {
    unsafe fn drop_event_packet(evt: *mut EvtPacket) {
        let stub = {
            let p_evt_stub = &(*evt).evt_serial as *const _ as *const EvtStub;
            ptr::read_volatile(p_evt_stub)
        };

        if !(stub.evt_code == TL_BLEEVT_CC_OPCODE || stub.evt_code == TL_BLEEVT_CS_OPCODE) {
            MemoryManager::drop_event_packet(evt);
        }
    }
}

pub fn to_err(e: bt_hci::FromHciBytesError) -> embedded_io::ErrorKind {
    use bt_hci::FromHciBytesError;
    use embedded_io::ErrorKind;

    match e {
        FromHciBytesError::InvalidSize => ErrorKind::InvalidInput,
        FromHciBytesError::InvalidValue => ErrorKind::InvalidData,
    }
}

pub fn make_cc_with_cs<'a>(
    buf: &'a [u8],
) -> Result<bt_hci::event::CommandCompleteWithStatus<'a>, bt_hci::cmd::Error<embedded_io::ErrorKind>>
{
    use bt_hci::cmd::Error as CmdError;
    use bt_hci::event::{CommandComplete, CommandCompleteWithStatus, CommandStatus, EventKind};
    use bt_hci::param::RemainingBytes;
    use bt_hci::{ControllerToHostPacket, FromHciBytes};
    use embedded_io::ErrorKind;

    let (pkt, _) = ControllerToHostPacket::from_hci_bytes(buf)
        .map_err(to_err)
        .map_err(CmdError::Io)?;

    let ControllerToHostPacket::Event(ref event) = pkt else {
        return Err(CmdError::Io(ErrorKind::InvalidData));
    };

    match event.kind {
        EventKind::CommandComplete => {
            let e = CommandComplete::from_hci_bytes_complete(event.data)
                .map_err(to_err)
                .map_err(CmdError::Io)?;

            e.try_into().map_err(to_err).map_err(CmdError::Io)
        }
        EventKind::CommandStatus => {
            let e = CommandStatus::from_hci_bytes_complete(event.data)
                .map_err(to_err)
                .map_err(CmdError::Io)?;

            Ok(CommandCompleteWithStatus {
                num_hci_cmd_pkts: 0,
                cmd_opcode: e.cmd_opcode,
                status: e.status,
                return_param_bytes: RemainingBytes::default(),
            })
        }
        _ => Err(CmdError::Io(ErrorKind::InvalidData)),
    }
}

pub struct ControllerAdapter<'d> {
    hw_ipcc_ble_cmd_channel: Mutex<NoopRawMutex, IpccTxChannel<'d>>,
    ipcc_ble_event_channel: Mutex<NoopRawMutex, IpccRxChannel<'d>>,
    slot: blocking_mutex::NoopMutex<RefCell<Option<bt_hci::cmd::Opcode>>>,
    signal: Signal<NoopRawMutex, Option<EvtBox<Ble<'d>>>>,
    waker: AtomicWaker,
    pending_evt: blocking_mutex::NoopMutex<RefCell<Option<EvtBox<Ble<'d>>>>>,
    cc_no_status: AtomicBool,
}

impl<'d> embedded_io::ErrorType for ControllerAdapter<'d> {
    type Error = embedded_io::ErrorKind;
}

struct SlotGuard<'d, 'a> {
    controller: &'a ControllerAdapter<'d>,
}

impl<'d, 'a> Drop for SlotGuard<'d, 'a> {
    fn drop(&mut self) {
        self.controller.slot.borrow().borrow_mut().take();
        self.controller.signal.reset();
        self.controller.waker.wake();
    }
}

impl<'d> ControllerAdapter<'d> {
    pub fn new(controller: Ble<'d>) -> Self {
        Self {
            hw_ipcc_ble_cmd_channel: Mutex::new(controller.cmd),
            ipcc_ble_event_channel: Mutex::new(controller.evt),
            slot: blocking_mutex::NoopMutex::const_new(NoopRawMutex::new(), RefCell::new(None)),
            signal: Signal::new(),
            waker: AtomicWaker::new(),
            pending_evt: blocking_mutex::NoopMutex::const_new(
                NoopRawMutex::new(),
                RefCell::new(None),
            ),
            cc_no_status: AtomicBool::new(false),
        }
    }

    async fn grab_slot<'a>(&'a self, opcode: bt_hci::cmd::Opcode) -> SlotGuard<'d, 'a> {
        use core::task::Poll;

        poll_fn(|cx| {
            self.waker.register(cx.waker());

            let mut slot = self.slot.borrow().borrow_mut();

            if slot.is_some() || self.cc_no_status.load(Ordering::Acquire) {
                Poll::Pending
            } else {
                *slot = Some(opcode);

                Poll::Ready(SlotGuard { controller: self })
            }
        })
        .await
    }

    fn signal_cmd(&self, opcode: bt_hci::cmd::Opcode, evt: EvtBox<Ble<'d>>) {
        use bt_hci::cmd::Cmd;
        use bt_hci::cmd::controller_baseband::Reset;

        let slot = self.slot.borrow().borrow_mut();

        if let Some(waiting_opcode) = *slot
            && waiting_opcode == opcode
        {
            self.signal.signal(Some(evt));
        } else if slot.is_some() && opcode == Reset::OPCODE {
            self.signal.signal(None);
        }
    }

    async fn read_pkt(
        &self,
    ) -> Result<(EvtBox<Ble<'d>>, bt_hci::ControllerToHostPacket<'static>), embedded_io::ErrorKind>
    {
        use bt_hci::{ControllerToHostPacket, FromHciBytes};

        let evt: EvtBox<Ble<'d>> = self
            .ipcc_ble_event_channel
            .lock()
            .await
            .receive(|| unsafe {
                critical_section::with(|cs| LinkedListNode::remove_head(cs, EVT_QUEUE.as_mut_ptr()))
                    .map(|node_ptr| EvtBox::new(node_ptr.cast()))
            })
            .await;

        let serial = evt.serial();
        let buf =
            unsafe { core::slice::from_raw_parts(serial as *const _ as *const u8, serial.len()) };

        debug!("receive: {:x}", buf[..buf.len().min(20)]);

        Ok((
            evt,
            ControllerToHostPacket::from_hci_bytes(buf)
                .map_err(to_err)
                .map(|(pkt, _)| pkt)?,
        ))
    }

    async fn exec_cmd<C: bt_hci::WriteHci + bt_hci::cmd::Cmd>(
        &self,
        cmd: &C,
    ) -> Result<
        (
            bt_hci::event::CommandCompleteWithStatus<'_>,
            SlotGuard<'d, '_>,
        ),
        bt_hci::cmd::Error<embedded_io::ErrorKind>,
    > {
        use bt_hci::cmd::Error as CmdError;
        use bt_hci::param::Error as ParamError;

        let guard = self.grab_slot(C::OPCODE).await;

        let mut ret = Ok(());
        self.hw_ipcc_ble_cmd_channel
            .lock()
            .await
            .send(|| unsafe {
                ret = WithIndicator::new(cmd)
                    .write_hci(CmdPacket::writer(BLE_CMD_BUFFER.as_mut_ptr()))
                    .map_err(CmdError::Io);
            })
            .await;

        ret?;

        let evt = self
            .signal
            .wait()
            .await
            .ok_or(CmdError::Hci(ParamError::OPERATION_CANCELLED_BY_HOST))?;

        // Packets with CC or CS opcode are not managed by the memory manager
        let evt_serial = unsafe {
            slice::from_raw_parts(evt.serial() as *const _ as *const u8, evt.serial().len())
        };

        Ok((make_cc_with_cs(evt_serial)?, guard))
    }
}

impl<'d> bt_hci::controller::Controller for ControllerAdapter<'d> {
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
        use bt_hci::event::{CommandComplete, CommandStatus, EventKind};
        use bt_hci::{ControllerToHostPacket, FromHciBytes};
        loop {
            // Drop the pending evt so that the memory manager can clean it up
            self.pending_evt.borrow().borrow_mut().take();
            if self.cc_no_status.swap(false, Ordering::AcqRel) {
                self.waker.wake();
            }

            let (evt, pkt) = self.read_pkt().await?;

            match pkt {
                ControllerToHostPacket::Event(ref event) => match event.kind {
                    EventKind::CommandComplete => {
                        let e =
                            CommandComplete::from_hci_bytes_complete(event.data).map_err(to_err)?;
                        if !e.has_status() {
                            // Store the pending event and block commands until the next read
                            self.cc_no_status.store(true, Ordering::Release);
                            self.pending_evt.borrow().borrow_mut().replace(evt);

                            return Ok(pkt);
                        }

                        self.signal_cmd(e.cmd_opcode, evt);
                        continue;
                    }
                    EventKind::CommandStatus => {
                        let e =
                            CommandStatus::from_hci_bytes_complete(event.data).map_err(to_err)?;

                        self.signal_cmd(e.cmd_opcode, evt);
                        continue;
                    }
                    _ => {
                        // Store the pending event so that it isn't dropped until the next read
                        self.pending_evt.borrow().borrow_mut().replace(evt);

                        return Ok(pkt);
                    }
                },
                _ => {
                    // Store the pending event so that it isn't dropped until the next read
                    self.pending_evt.borrow().borrow_mut().replace(evt);

                    return Ok(pkt);
                }
            }
        }
    }
}

impl<'d, C> bt_hci::controller::ControllerCmdSync<C> for ControllerAdapter<'d>
where
    C: bt_hci::cmd::SyncCmd,
{
    async fn exec(&self, cmd: &C) -> Result<C::Return, bt_hci::cmd::Error<Self::Error>> {
        use bt_hci::cmd;

        debug!("Executing sync command with opcode {:x}", C::OPCODE.0);

        let (e, _guard) = self.exec_cmd(cmd).await?;

        trace!("returned ccws: {:?}", e.status);

        let r = e.to_result::<C>().map_err(cmd::Error::Hci)?;
        debug!("Done executing command with opcode {:x}", C::OPCODE.0);
        Ok(r)
    }
}

impl<'d, C> bt_hci::controller::ControllerCmdAsync<C> for ControllerAdapter<'d>
where
    C: bt_hci::cmd::AsyncCmd,
{
    async fn exec(&self, cmd: &C) -> Result<(), bt_hci::cmd::Error<Self::Error>> {
        debug!("Executing async command with opcode {:x}", C::OPCODE.0);

        let (e, _guard) = self.exec_cmd(cmd).await?;

        trace!("returned ccws: {:?}", e.status);

        e.status.to_result()?;

        debug!("Done executing command with opcode {:x}", C::OPCODE.0);
        Ok(())
    }
}

pub struct MemoryManager<'a> {
    release: IpccTxChannel<'a>,
}

static MM_WAKER: AtomicWaker = AtomicWaker::new();
static mut LOCAL_FREE_BUF_QUEUE: Aligned<A4, MaybeUninit<LinkedListNode>> =
    Aligned(MaybeUninit::uninit());

impl<'a> MemoryManager<'a> {
    fn new(release: IpccTxChannel<'a>) -> Self {
        unsafe {
            LinkedListNode::init_head(FREE_BUF_QUEUE.as_mut_ptr());
            LinkedListNode::init_head(LOCAL_FREE_BUF_QUEUE.as_mut_ptr());
            TL_MEM_MANAGER_TABLE
                .as_mut_ptr()
                .write_volatile(MemManagerTable {
                    spare_ble_buffer: BLE_SPARE_EVT_BUF.as_ptr().cast(),
                    spare_sys_buffer: SYS_SPARE_EVT_BUF.as_ptr().cast(),
                    blepool: EVT_POOL.as_ptr().cast(),
                    blepoolsize: POOL_SIZE as u32,
                    pevt_free_buffer_queue: FREE_BUF_QUEUE.as_mut_ptr(),
                    traces_evt_pool: ptr::null(),
                    tracespoolsize: 0,
                });
        }

        Self { release }
    }

    pub async fn run_queue(&mut self) -> ! {
        loop {
            // Drop handlers enqueue released buffers locally; this task hands them back to CPU2.
            poll_fn(|cx| unsafe {
                MM_WAKER.register(cx.waker());
                if critical_section::with(|cs| {
                    LinkedListNode::is_empty(cs, LOCAL_FREE_BUF_QUEUE.as_mut_ptr())
                }) {
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;

            self.release
                .send(|| {
                    critical_section::with(|cs| unsafe {
                        while let Some(node) =
                            LinkedListNode::remove_head(cs, LOCAL_FREE_BUF_QUEUE.as_mut_ptr())
                        {
                            LinkedListNode::insert_head(cs, FREE_BUF_QUEUE.as_mut_ptr(), node);
                        }
                    })
                })
                .await;
        }
    }

    unsafe fn drop_event_packet(evt: *mut EvtPacket) {
        critical_section::with(|cs| {
            LinkedListNode::insert_head(cs, LOCAL_FREE_BUF_QUEUE.as_mut_ptr(), evt as *mut _);
        });

        MM_WAKER.wake();
    }
}

impl<'a> EventMemoryManager for MemoryManager<'a> {
    unsafe fn drop_event_packet(evt: *mut EvtPacket) {
        Self::drop_event_packet(evt);
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct BleInitParam {
    p_ble_buffer_address: u32,
    ble_buffer_size: u32,
    num_attr_record: u16,
    num_attr_serv: u16,
    attr_value_arr_size: u16,
    num_of_links: u8,
    extended_packet_length_enable: u8,
    prepare_write_list_size: u8,
    block_count: u8,
    att_mtu: u16,
    slave_sca: u16,
    master_sca: u8,
    ls_source: u8,
    max_conn_event_length: u32,
    hs_startup_time: u16,
    viterbi_enable: u8,
    options: u8,
    hw_version: u8,
    max_coc_initiator_nbr: u8,
    min_tx_power: i8,
    max_tx_power: i8,
    rx_model_config: u8,
    max_adv_set_nbr: u8,
    max_adv_data_len: u16,
    tx_path_compens: i16,
    rx_path_compens: i16,
    ble_core_version: u8,
    options_extension: u8,
    max_add_eatt_bearers: u8,
}

impl BleInitParam {
    fn payload(&self) -> &[u8] {
        // SHCI command parameters are packed C ABI bytes defined by STM32WB firmware.
        unsafe { slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>()) }
    }
}

impl Default for BleInitParam {
    fn default() -> Self {
        Self {
            p_ble_buffer_address: 0,
            ble_buffer_size: 0,
            num_attr_record: 68,
            num_attr_serv: 4,
            attr_value_arr_size: 1344,
            num_of_links: 2,
            extended_packet_length_enable: 1,
            prepare_write_list_size: 0x3A,
            block_count: 0x79,
            att_mtu: 156,
            slave_sca: 500,
            master_sca: 0,
            ls_source: 1,
            max_conn_event_length: 0xFFFF_FFFF,
            hs_startup_time: 0x148,
            viterbi_enable: 1,
            options: 0,
            hw_version: 0,
            max_coc_initiator_nbr: 32,
            min_tx_power: -40,
            max_tx_power: 6,
            rx_model_config: 0,
            max_adv_set_nbr: 2,
            max_adv_data_len: 1650,
            tx_path_compens: 0,
            rx_path_compens: 0,
            ble_core_version: 11,
            options_extension: 0,
            max_add_eatt_bearers: 4,
        }
    }
}

const SHCI_OPCODE_BLE_INIT: u16 = shci_opcode(0x66);

#[derive(Clone, Copy, defmt::Format)]
#[allow(clippy::enum_variant_names)]
pub enum SysEventReady {
    WirelessFwRunning = 0x00,
    FusFwRunning = 0x01,
    NvmBackupRunning = 0x10,
    NvmRestoreRunning = 0x11,
}

impl TryFrom<u8> for SysEventReady {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(Self::WirelessFwRunning),
            0x01 => Ok(Self::FusFwRunning),
            0x10 => Ok(Self::NvmBackupRunning),
            0x11 => Ok(Self::NvmRestoreRunning),
            _ => Err(()),
        }
    }
}

#[allow(dead_code)]
pub enum SysCommandStatus {
    Success = 0x00,
    UnknownCommand = 0x01,
    MemoryCapacityExceeded = 0x07,
    UnsupportedFeature = 0x11,
    InvalidHciCommandParams = 0x12,
    InvalidParams = 0x42,
    InvalidParamsV2 = 0x92,
    FusCommandNotSupported = 0xFF,
}

impl SysCommandStatus {
    unsafe fn from_cmd_buffer(cmd_buf: *const CmdPacket) -> Result<Self, ()> {
        let p_cmd_serial = (cmd_buf as *mut u8).add(mem::size_of::<LinkedListNode>());
        let p_evt_payload = p_cmd_serial.add(mem::size_of::<EvtStub>());

        compiler_fence(Ordering::Acquire);
        let cc_evt = ptr::read_unaligned(p_evt_payload as *const CcEvt);
        cc_evt.payload[0].try_into()
    }
}

impl TryFrom<u8> for SysCommandStatus {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(Self::Success),
            0x01 => Ok(Self::UnknownCommand),
            0x07 => Ok(Self::MemoryCapacityExceeded),
            0x11 => Ok(Self::UnsupportedFeature),
            0x12 => Ok(Self::InvalidHciCommandParams),
            0x42 => Ok(Self::InvalidParams),
            0x92 => Ok(Self::InvalidParamsV2),
            0xFF => Ok(Self::FusCommandNotSupported),
            _ => Err(()),
        }
    }
}

#[repr(u8)]
enum TlPacketType {
    _BleCmd = 0x01,
    _AclData = 0x02,
    SysCmd = 0x10,
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed(4))]
struct LinkedListNode {
    next: *mut LinkedListNode,
    prev: *mut LinkedListNode,
}

impl LinkedListNode {
    unsafe fn init_head(head: *mut LinkedListNode) {
        ptr::write_volatile(
            head,
            LinkedListNode {
                next: head,
                prev: head,
            },
        );
    }

    unsafe fn is_empty(_cs: critical_section::CriticalSection, head: *mut LinkedListNode) -> bool {
        ptr::read_volatile(head).next == head
    }

    unsafe fn insert_head(
        _cs: critical_section::CriticalSection,
        head: *mut LinkedListNode,
        node: *mut LinkedListNode,
    ) {
        let mut list_head = ptr::read_volatile(head);
        if head != list_head.next {
            let mut node_next = ptr::read_volatile(list_head.next);
            let new_node = LinkedListNode {
                next: list_head.next,
                prev: head,
            };

            list_head.next = node;
            node_next.prev = node;

            ptr::write_volatile(node, new_node);
            ptr::write_volatile(new_node.next, node_next);
            ptr::write_volatile(head, list_head);
        } else {
            let new_node = LinkedListNode {
                next: list_head.next,
                prev: head,
            };

            list_head.next = node;
            list_head.prev = node;

            ptr::write_volatile(node, new_node);
            ptr::write_volatile(head, list_head);
        }
    }

    unsafe fn remove_head(
        cs: critical_section::CriticalSection,
        head: *mut LinkedListNode,
    ) -> Option<*mut LinkedListNode> {
        let list_head = ptr::read_volatile(head);
        if list_head.next == head {
            None
        } else {
            let node = list_head.next;
            Self::remove_node(cs, node);
            Some(node)
        }
    }

    unsafe fn remove_node(_cs: critical_section::CriticalSection, node: *mut LinkedListNode) {
        let node_value = ptr::read_unaligned(node);

        if node_value.next != node_value.prev {
            let mut next = ptr::read_volatile(node_value.next);
            let mut prev = ptr::read_volatile(node_value.prev);
            prev.next = node_value.next;
            next.prev = node_value.prev;

            ptr::write_volatile(node_value.next, next);
            ptr::write_volatile(node_value.prev, prev);
        } else {
            let mut next = ptr::read_volatile(node_value.next);
            next.next = node_value.next;
            next.prev = node_value.prev;

            ptr::write_volatile(node_value.next, next);
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Cmd {
    cmd_code: u16,
    payload_len: u8,
    payload: [u8; 255],
}

impl Default for Cmd {
    fn default() -> Self {
        Self {
            cmd_code: 0,
            payload_len: 0,
            payload: [0; 255],
        }
    }
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct CmdSerial {
    ty: u8,
    cmd: Cmd,
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct CmdSerialStub {
    ty: u8,
    cmd_code: u16,
    payload_len: u8,
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct CmdPacket {
    header: LinkedListNode,
    cmdserial: CmdSerial,
}

impl CmdPacket {
    unsafe fn write_into(
        buf: *mut CmdPacket,
        packet_type: TlPacketType,
        cmd_code: u16,
        payload: &[u8],
    ) {
        let p_cmd_serial = (buf as *mut u8).add(mem::size_of::<LinkedListNode>());
        let p_payload = p_cmd_serial.add(mem::size_of::<CmdSerialStub>());

        ptr::write_unaligned(
            p_cmd_serial as *mut _,
            CmdSerialStub {
                ty: packet_type as u8,
                cmd_code,
                payload_len: payload.len() as u8,
            },
        );
        ptr::copy_nonoverlapping(payload.as_ptr(), p_payload, payload.len());

        compiler_fence(Ordering::Release);
    }

    pub unsafe fn writer(cmd_buf: *mut CmdPacket) -> VolatileWriter {
        let p_cmd_serial = (cmd_buf as *mut u8).add(size_of::<LinkedListNode>());

        VolatileWriter {
            start: p_cmd_serial,
            len: 255,
        }
    }
}

impl embedded_io::ErrorType for VolatileWriter {
    type Error = embedded_io::ErrorKind;
}

pub struct VolatileWriter {
    start: *mut u8,
    len: usize,
}

impl embedded_io::Write for VolatileWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        use embedded_io::ErrorKind;

        if self.len == 0 {
            return Err(ErrorKind::WriteZero);
        }

        unsafe {
            let count = self.len.min(buf.len());

            ptr::copy_nonoverlapping(buf as *const _ as *const u8, self.start, count);

            self.start = self.start.add(count);
            self.len -= count;

            Ok(count)
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        compiler_fence(Ordering::Release);

        Ok(())
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct AclDataPacket {
    header: LinkedListNode,
    acl_data_serial: AclDataSerial,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct AclDataSerial {
    ty: u8,
    handle: u16,
    length: u16,
    acl_data: [u8; 1],
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct CsEvt {
    status: u8,
    num_cmd: u8,
    cmd_code: u16,
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct CcEvt {
    num_cmd: u8,
    cmd_code: u16,
    payload: [u8; 1],
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Evt {
    evt_code: u8,
    payload_len: u8,
    payload: [u8; 255],
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct EvtSerial {
    kind: u8,
    evt: Evt,
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct EvtStub {
    kind: u8,
    evt_code: u8,
    payload_len: u8,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct EvtPacket {
    header: LinkedListNode,
    evt_serial: EvtSerial,
}

trait EventMemoryManager {
    unsafe fn drop_event_packet(evt: *mut EvtPacket);
}

struct EvtBox<T: EventMemoryManager> {
    ptr: *mut EvtPacket,
    mm: PhantomData<T>,
}

unsafe impl<T: EventMemoryManager> Send for EvtBox<T> {}

impl<T: EventMemoryManager> EvtBox<T> {
    fn new(ptr: *mut EvtPacket) -> Self {
        Self {
            ptr,
            mm: PhantomData,
        }
    }

    fn payload(&self) -> &[u8] {
        unsafe {
            let p_len = &(*self.ptr).evt_serial.evt.payload_len as *const u8;
            let p_payload = &(*self.ptr).evt_serial.evt.payload as *const u8;
            slice::from_raw_parts(p_payload, ptr::read_volatile(p_len) as usize)
        }
    }

    fn serial(&self) -> &[u8] {
        unsafe {
            let evt_serial: *const EvtSerial = &(*self.ptr).evt_serial;
            let len = (*evt_serial).evt.payload_len as usize + TL_EVT_HEADER_SIZE;
            slice::from_raw_parts(evt_serial.cast(), len)
        }
    }
}

impl<T: EventMemoryManager> Drop for EvtBox<T> {
    fn drop(&mut self) {
        unsafe { T::drop_event_packet(self.ptr) };
    }
}

#[repr(C)]
struct DeviceInfoTable {
    safe_boot_info_table: [u32; 1],
    rss_info_table: [u32; 3],
    wireless_fw_info_table: [u32; 4],
}

#[repr(C)]
struct BleTable {
    pcmd_buffer: *mut CmdPacket,
    pcs_buffer: *const u8,
    pevt_queue: *const u8,
    phci_acl_data_buffer: *mut AclDataPacket,
}

#[repr(C)]
struct ThreadTable {
    nostack_buffer: *const u8,
    clicmdrsp_buffer: *const u8,
    otcmdrsp_buffer: *const u8,
}

#[repr(C)]
struct SysTable {
    pcmd_buffer: *mut CmdPacket,
    sys_queue: *const LinkedListNode,
}

#[repr(C)]
struct MemManagerTable {
    spare_ble_buffer: *const u8,
    spare_sys_buffer: *const u8,
    blepool: *const u8,
    blepoolsize: u32,
    pevt_free_buffer_queue: *mut LinkedListNode,
    traces_evt_pool: *const u8,
    tracespoolsize: u32,
}

#[repr(C)]
struct TracesTable {
    traces_queue: *const u8,
}

#[repr(C)]
struct Mac802_15_4Table {
    p_cmdrsp_buffer: *const u8,
    p_notack_buffer: *const u8,
    evt_queue: *const u8,
}

#[repr(C)]
struct ZigbeeTable {
    notif_m0_to_m4_buffer: *const u8,
    appli_cmd_m4_to_m0_bufer: *const u8,
    request_m0_to_m4_buffer: *const u8,
}

#[repr(C)]
struct LldTestsTable {
    clicmdrsp_buffer: *const u8,
    m0cmd_buffer: *const u8,
}

#[repr(C)]
struct BleLldTable {
    cmdrsp_buffer: *const u8,
    m0cmd_buffer: *const u8,
}

#[repr(C)]
struct RefTable {
    device_info_table: *const DeviceInfoTable,
    ble_table: *const BleTable,
    thread_table: *const ThreadTable,
    sys_table: *const SysTable,
    mem_manager_table: *const MemManagerTable,
    traces_table: *const TracesTable,
    mac_802_15_4_table: *const Mac802_15_4Table,
    zigbee_table: *const ZigbeeTable,
    lld_tests_table: *const LldTestsTable,
    ble_lld_table: *const BleLldTable,
}

#[unsafe(link_section = "TL_REF_TABLE")]
static mut TL_REF_TABLE: MaybeUninit<RefTable> = MaybeUninit::zeroed();

#[unsafe(link_section = "MB_MEM1")]
static mut TL_DEVICE_INFO_TABLE: Aligned<A4, MaybeUninit<DeviceInfoTable>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_BLE_TABLE: Aligned<A4, MaybeUninit<BleTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_THREAD_TABLE: Aligned<A4, MaybeUninit<ThreadTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_SYS_TABLE: Aligned<A4, MaybeUninit<SysTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_MEM_MANAGER_TABLE: Aligned<A4, MaybeUninit<MemManagerTable>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_TRACES_TABLE: Aligned<A4, MaybeUninit<TracesTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_MAC_802_15_4_TABLE: Aligned<A4, MaybeUninit<Mac802_15_4Table>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_ZIGBEE_TABLE: Aligned<A4, MaybeUninit<ZigbeeTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_LLD_TESTS_TABLE: Aligned<A4, MaybeUninit<LldTestsTable>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut TL_BLE_LLD_TABLE: Aligned<A4, MaybeUninit<BleLldTable>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM1")]
static mut BLE_CMD_BUFFER: Aligned<A4, MaybeUninit<CmdPacket>> = Aligned(MaybeUninit::zeroed());

#[unsafe(link_section = "MB_MEM2")]
static mut FREE_BUF_QUEUE: Aligned<A4, MaybeUninit<LinkedListNode>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut CS_BUFFER: Aligned<
    A4,
    MaybeUninit<[u8; TL_PACKET_HEADER_SIZE + TL_EVT_HEADER_SIZE + TL_CS_EVT_SIZE]>,
> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut EVT_QUEUE: Aligned<A4, MaybeUninit<LinkedListNode>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut SYSTEM_EVT_QUEUE: Aligned<A4, MaybeUninit<LinkedListNode>> =
    Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut EVT_POOL: Aligned<A4, MaybeUninit<[u8; POOL_SIZE]>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut SYS_CMD_BUF: Aligned<A4, MaybeUninit<CmdPacket>> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut SYS_SPARE_EVT_BUF: Aligned<
    A4,
    MaybeUninit<[u8; TL_PACKET_HEADER_SIZE + TL_EVT_HEADER_SIZE + 255]>,
> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut BLE_SPARE_EVT_BUF: Aligned<
    A4,
    MaybeUninit<[u8; TL_PACKET_HEADER_SIZE + TL_EVT_HEADER_SIZE + 255]>,
> = Aligned(MaybeUninit::zeroed());
#[unsafe(link_section = "MB_MEM2")]
static mut HCI_ACL_DATA_BUFFER: Aligned<A4, MaybeUninit<[u8; TL_PACKET_HEADER_SIZE + 5 + 251]>> =
    Aligned(MaybeUninit::zeroed());
