//! IPv4 sockets serviced by the boot CPU on the QEMU user network.
use core::{mem::MaybeUninit, ptr::NonNull};

use lumeth::{DMA_PAGES, FRAME_CAPACITY, Rtl8139};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::{tcp, udp},
    time::Instant,
    wire::{
        EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, IpListenEndpoint,
        Ipv4Address,
    },
};

use super::{arch, memory, syscalls::ticks};
use crate::{abi, bindings, pci};

const ADDRESS: Ipv4Address = Ipv4Address::new(10, 0, 2, 15);
const GATEWAY: Ipv4Address = Ipv4Address::new(10, 0, 2, 2);
const SLOTS: usize = 12;
const BUFFER: usize = 8192;
const PAYLOAD: usize = 1472;
const EAGAIN: i64 = -11;
const EFAULT: i64 = -14;
const EINVAL: i64 = -22;
const EMSGSIZE: i64 = -90;
const EADDRINUSE: i64 = -98;
const EADDRNOTAVAIL: i64 = -99;
const ENETDOWN: i64 = -100;
const ENETUNREACH: i64 = -101;
const EISCONN: i64 = -106;
const ENOTCONN: i64 = -107;
const ETIMEDOUT: i64 = -110;
const ECONNREFUSED: i64 = -111;
const EINPROGRESS: i64 = -115;
const POLLIN: u16 = 1;
const POLLOUT: u16 = 4;
const POLLHUP: u16 = 16;

struct Ethernet(Rtl8139);

struct Receive {
    data: [u8; FRAME_CAPACITY],
    size: usize,
}

struct Transmit<'a>(&'a mut Rtl8139);

impl RxToken for Receive {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, callback: F) -> R {
        callback(&self.data[..self.size])
    }
}

impl TxToken for Transmit<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, callback: F) -> R {
        let mut frame = [0; FRAME_CAPACITY];
        let result = callback(&mut frame[..len]);
        self.0.transmit(&frame[..len]);
        result
    }
}

impl Device for Ethernet {
    type RxToken<'a> = Receive;
    type TxToken<'a> = Transmit<'a>;

    fn receive(&mut self, _now: Instant) -> Option<(Receive, Transmit<'_>)> {
        if !self.0.can_transmit() {
            return None;
        }
        let mut data = [0; FRAME_CAPACITY];
        let size = self.0.receive(&mut data)?;
        Some((Receive { data, size }, Transmit(&mut self.0)))
    }

    fn transmit(&mut self, _now: Instant) -> Option<Transmit<'_>> {
        self.0.can_transmit().then_some(Transmit(&mut self.0))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = Some(4);
        caps
    }
}

struct Buffers {
    inbound: [u8; BUFFER],
    outbound: [u8; BUFFER],
    rx_packets: [udp::PacketMetadata; 8],
    tx_packets: [udp::PacketMetadata; 8],
}

struct Network {
    interface: Interface,
    device: Ethernet,
    sockets: SocketSet<'static>,
    occupied: [bool; SLOTS],
    bound: [Option<(bool, IpListenEndpoint)>; SLOTS],
    closing: [Option<(SocketHandle, u64)>; SLOTS],
    next_port: u16,
}

static mut NETWORK: Option<Network> = None;
static mut STORAGE: MaybeUninit<[SocketStorage<'static>; SLOTS]> = MaybeUninit::uninit();
static mut BUFFERS: [MaybeUninit<Buffers>; SLOTS] = [const { MaybeUninit::uninit() }; SLOTS];

#[derive(Clone, Copy)]
pub struct Socket {
    tcp: bool,
    slot: usize,
    handle: SocketHandle,
    pub nonblocking: bool,
    pub reuse_address: bool,
    pub read_timeout: u64,
    pub write_timeout: u64,
    pub local: IpListenEndpoint,
    peer: Option<IpEndpoint>,
}

#[unsafe(no_mangle)]
extern "C" fn lumen_network_init() {
    let Some(card) = pci::locate(0x10ec, 0x8139) else {
        return;
    };
    let bar = card.read(0x10);
    if bar & 1 == 0 || !(1..=0xff00).contains(&(bar & !3)) {
        return;
    }

    let Some(dma) = NonNull::new(unsafe { bindings::kpalloc_contiguous(DMA_PAGES as u64) }) else {
        return;
    };
    let Ok(physical) = u32::try_from(dma.as_ptr().addr()) else {
        unsafe { bindings::kpfree_contiguous(dma.as_ptr(), DMA_PAGES as u64) };
        return;
    };
    unsafe { dma.as_ptr().write_bytes(0, DMA_PAGES * 4096) };
    card.enable(0x405); // I/O decoding and bus mastering, with INTx disabled
    let Some(card) = (unsafe { Rtl8139::start((bar & !3) as u16, dma, physical) }) else {
        unsafe { bindings::kpfree_contiguous(dma.as_ptr(), DMA_PAGES as u64) };
        return;
    };

    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(card.mac())));
    let seed = arch::random().unwrap_or_else(arch::tsc);
    config.random_seed = seed;
    let mut device = Ethernet(card);
    let mut interface = Interface::new(config, &mut device, now());
    interface.update_ip_addrs(|addresses| {
        let _ = addresses.push(IpCidr::new(IpAddress::Ipv4(ADDRESS), 24));
    });
    let _ = interface.routes_mut().add_default_ipv4_route(GATEWAY);

    unsafe {
        let storage = (&mut *(&raw mut STORAGE)).write([SocketStorage::EMPTY; SLOTS]);
        NETWORK = Some(Network {
            interface,
            device,
            sockets: SocketSet::new(storage.as_mut_slice()),
            occupied: [false; SLOTS],
            bound: [None; SLOTS],
            closing: [None; SLOTS],
            next_port: 49152 + ((seed as u16) & 0x3fff),
        });
        crate::console::BootConsole::new().write("Network: RTL8139 at 10.0.2.15/24\n");
    }
}

fn now() -> Instant {
    Instant::from_millis(ticks() as i64)
}

fn with<T>(use_network: impl FnOnce(&mut Network) -> T) -> Option<T> {
    // Every call runs on the boot CPU with interrupts masked. The callback
    // completes before any sleep or scheduler handoff.
    unsafe { (&mut *(&raw mut NETWORK)).as_mut().map(use_network) }
}

impl Network {
    fn poll(&mut self) {
        self.interface
            .poll(now(), &mut self.device, &mut self.sockets);
        for slot in 0..SLOTS {
            if let Some((handle, deadline)) = self.closing[slot]
                && (self.sockets.get::<tcp::Socket>(handle).state() == tcp::State::Closed
                    || ticks() >= deadline)
            {
                self.sockets.remove(handle);
                self.occupied[slot] = false;
                self.bound[slot] = None;
                self.closing[slot] = None;
            }
        }
    }

    fn port(&mut self) -> u16 {
        loop {
            let port = self.next_port;
            self.next_port = if port == u16::MAX { 49152 } else { port + 1 };
            if self
                .bound
                .iter()
                .flatten()
                .all(|(_, address)| address.port != port)
            {
                return port;
            }
        }
    }

    fn allocate(&mut self, is_tcp: bool, nonblocking: bool) -> Result<Socket, i64> {
        let Some(slot) = self.occupied.iter().position(|flag| !flag) else {
            return Err(-23);
        };
        let buffers = unsafe {
            let storage = (&raw mut BUFFERS).cast::<MaybeUninit<Buffers>>().add(slot);
            let buffer = (*storage).as_mut_ptr();
            (&raw mut (*buffer).inbound).write([0; BUFFER]);
            (&raw mut (*buffer).outbound).write([0; BUFFER]);
            (&raw mut (*buffer).rx_packets).write([udp::PacketMetadata::EMPTY; 8]);
            (&raw mut (*buffer).tx_packets).write([udp::PacketMetadata::EMPTY; 8]);
            &mut *buffer
        };
        let handle = if is_tcp {
            self.sockets.add(tcp::Socket::new(
                tcp::SocketBuffer::new(buffers.inbound.as_mut_slice()),
                tcp::SocketBuffer::new(buffers.outbound.as_mut_slice()),
            ))
        } else {
            self.sockets.add(udp::Socket::new(
                udp::PacketBuffer::new(
                    buffers.rx_packets.as_mut_slice(),
                    buffers.inbound.as_mut_slice(),
                ),
                udp::PacketBuffer::new(
                    buffers.tx_packets.as_mut_slice(),
                    buffers.outbound.as_mut_slice(),
                ),
            ))
        };

        self.occupied[slot] = true;
        Ok(Socket {
            tcp: is_tcp,
            slot,
            handle,
            nonblocking,
            reuse_address: false,
            read_timeout: 0,
            write_timeout: 0,
            local: IpListenEndpoint::default(),
            peer: None,
        })
    }
}

pub fn poll() {
    with(Network::poll);
}

pub fn open(kind: u64, flags: u64) -> Result<Socket, i64> {
    if flags & !abi::NONBLOCK != 0 {
        return Err(EINVAL);
    }
    let tcp = match kind {
        abi::SOCKET_STREAM => true,
        abi::SOCKET_DATAGRAM => false,
        _ => return Err(-93),
    };
    with(|network| network.allocate(tcp, flags & abi::NONBLOCK != 0)).ok_or(ENETDOWN)?
}

impl Socket {
    pub fn is_stream(&self) -> bool {
        self.tcp
    }

    pub fn pending_error(&self) -> i32 {
        if !self.tcp || self.peer.is_none() {
            return 0;
        }
        with(|network| {
            network.poll();
            if network.sockets.get::<tcp::Socket>(self.handle).state() == tcp::State::Closed {
                -ECONNREFUSED as i32
            } else {
                0
            }
        })
        .unwrap_or(-ENETDOWN as i32)
    }

    pub fn close(self) {
        with(|network| {
            if self.tcp {
                let socket = network.sockets.get_mut::<tcp::Socket>(self.handle);
                socket.close();
                if socket.state() != tcp::State::Closed {
                    network.closing[self.slot] =
                        Some((self.handle, ticks().saturating_add(20_000)));
                    network.poll();
                    return;
                }
            }
            network.sockets.remove(self.handle);
            network.occupied[self.slot] = false;
            network.bound[self.slot] = None;
        });
    }

    pub fn bind(&mut self, endpoint: IpEndpoint) -> i64 {
        if endpoint.addr != IpAddress::Ipv4(ADDRESS) && !endpoint.addr.is_unspecified() {
            return EADDRNOTAVAIL;
        }
        if self.local.port != 0 {
            return EINVAL;
        }
        with(|network| {
            let port = if endpoint.port == 0 {
                network.port()
            } else {
                endpoint.port
            };
            let local = IpListenEndpoint {
                addr: (!endpoint.addr.is_unspecified()).then_some(endpoint.addr),
                port,
            };
            let overlaps = |other: IpListenEndpoint| {
                other.port == local.port
                    && (other.addr.is_none() || local.addr.is_none() || other.addr == local.addr)
            };
            if network.bound.iter().enumerate().any(|(slot, binding)| {
                binding.is_some_and(|(is_tcp, other)| {
                    is_tcp == self.tcp
                        && overlaps(other)
                        && !(self.reuse_address && network.closing[slot].is_some())
                })
            }) {
                return EADDRINUSE;
            }
            if !self.tcp {
                let udp = network.sockets.get_mut::<udp::Socket>(self.handle);
                if udp.bind(local).is_err() {
                    return EADDRINUSE;
                }
            }
            self.local = local;
            network.bound[self.slot] = Some((self.tcp, local));
            0
        })
        .unwrap_or(ENETDOWN)
    }

    pub fn connect(&mut self, endpoint: IpEndpoint) -> i64 {
        if endpoint.port == 0 || endpoint.addr.is_unspecified() {
            return EINVAL;
        }
        if !self.tcp {
            if self.local.port == 0 {
                let result = self.bind(IpEndpoint::new(
                    IpAddress::Ipv4(Ipv4Address::UNSPECIFIED),
                    0,
                ));
                if result != 0 {
                    return result;
                }
            }
            self.peer = Some(endpoint);
            return 0;
        }

        let start = with(|network| {
            if self.local.port == 0 {
                self.local.port = network.port();
                network.bound[self.slot] = Some((true, self.local));
            }
            let socket = network.sockets.get_mut::<tcp::Socket>(self.handle);
            match socket.connect(network.interface.context(), endpoint, self.local) {
                Ok(()) => {
                    self.peer = Some(endpoint);
                    network.poll();
                    0
                }
                Err(tcp::ConnectError::Unaddressable) => ENETUNREACH,
                Err(tcp::ConnectError::InvalidState) => EISCONN,
            }
        })
        .unwrap_or(ENETDOWN);
        if start != 0 {
            return start;
        }
        let deadline = ticks().saturating_add(10_000);
        loop {
            let state = with(|network| {
                network.poll();
                network.sockets.get::<tcp::Socket>(self.handle).state()
            });
            match state {
                Some(tcp::State::Established) => return 0,
                Some(tcp::State::Closed) => return ECONNREFUSED,
                None => return ENETDOWN,
                _ if self.nonblocking => return EINPROGRESS,
                _ if ticks() >= deadline => return ETIMEDOUT,
                _ => arch::wait_for_interrupt(),
            }
        }
    }

    pub fn listen(&mut self) -> i64 {
        if !self.tcp || self.local.port == 0 {
            return EINVAL;
        }
        with(|network| {
            if network
                .sockets
                .get_mut::<tcp::Socket>(self.handle)
                .listen(self.local)
                .is_err()
            {
                EADDRINUSE
            } else {
                0
            }
        })
        .unwrap_or(ENETDOWN)
    }

    pub fn accept(&mut self, nonblocking: bool) -> Result<Socket, i64> {
        if !self.tcp || self.peer.is_some() || self.local.port == 0 {
            return Err(EINVAL);
        }
        loop {
            let result = with(|network| {
                network.poll();
                let socket = network.sockets.get::<tcp::Socket>(self.handle);
                match socket.state() {
                    tcp::State::Established | tcp::State::CloseWait => {
                        let peer = socket.remote_endpoint();
                        let mut replacement = network.allocate(true, self.nonblocking)?;
                        replacement.local = self.local;
                        let listener = network.sockets.get_mut::<tcp::Socket>(replacement.handle);
                        if listener.listen(self.local).is_err() {
                            network.sockets.remove(replacement.handle);
                            network.occupied[replacement.slot] = false;
                            return Err(EADDRINUSE);
                        }
                        let mut accepted = core::mem::replace(self, replacement);
                        network.bound[self.slot] = network.bound[accepted.slot].take();
                        accepted.nonblocking = nonblocking;
                        accepted.peer = peer;
                        Ok(accepted)
                    }
                    tcp::State::Listen | tcp::State::SynReceived => Err(EAGAIN),
                    _ => Err(EINVAL),
                }
            })
            .ok_or(ENETDOWN)?;
            if !matches!(result, Err(EAGAIN)) || self.nonblocking {
                return result;
            }
            arch::wait_for_interrupt();
        }
    }

    pub fn address(&self, peer: bool) -> Result<IpEndpoint, i64> {
        if peer {
            return self.peer.ok_or(ENOTCONN);
        }
        let local = self.local.addr.unwrap_or_else(|| {
            IpAddress::Ipv4(if self.peer.is_some() {
                ADDRESS
            } else {
                Ipv4Address::UNSPECIFIED
            })
        });
        Ok(IpEndpoint::new(local, self.local.port))
    }

    pub fn send(&mut self, data: &[u8], destination: Option<IpEndpoint>, dontwait: bool) -> i64 {
        if !self.tcp && data.len() > PAYLOAD {
            return EMSGSIZE;
        }
        let peer = destination.or(self.peer);
        if peer.is_none() {
            return ENOTCONN;
        }
        if !self.tcp && self.local.port == 0 {
            let result = self.bind(IpEndpoint::new(
                IpAddress::Ipv4(Ipv4Address::UNSPECIFIED),
                0,
            ));
            if result != 0 {
                return result;
            }
        }
        let until = ticks().saturating_add(self.write_timeout);
        loop {
            let sent = with(|network| {
                network.poll();
                let result = if self.tcp {
                    let socket = network.sockets.get_mut::<tcp::Socket>(self.handle);
                    if !socket.may_send() {
                        return ENOTCONN;
                    }
                    socket
                        .send_slice(data)
                        .ok()
                        .filter(|&bytes| bytes != 0 || data.is_empty())
                        .map_or(EAGAIN, |bytes| bytes as i64)
                } else {
                    let udp = network.sockets.get_mut::<udp::Socket>(self.handle);
                    match udp.send_slice(data, peer.unwrap()) {
                        Ok(()) => data.len() as i64,
                        Err(udp::SendError::BufferFull) => EAGAIN,
                        Err(udp::SendError::Unaddressable) => EADDRNOTAVAIL,
                    }
                };
                network.poll();
                result
            })
            .unwrap_or(ENETDOWN);
            if sent != EAGAIN
                || self.nonblocking
                || dontwait
                || (self.write_timeout != 0 && ticks() >= until)
            {
                return sent;
            }
            arch::wait_for_interrupt();
        }
    }

    pub fn receive(
        &mut self,
        data: &mut [u8],
        dontwait: bool,
    ) -> Result<(usize, Option<IpEndpoint>), i64> {
        if self.tcp && data.is_empty() {
            return Ok((0, self.peer));
        }
        let until = ticks().saturating_add(self.read_timeout);
        loop {
            let result = with(|network| {
                network.poll();
                if self.tcp {
                    let socket = network.sockets.get_mut::<tcp::Socket>(self.handle);
                    if socket.can_recv() {
                        socket
                            .recv_slice(data)
                            .map(|size| (size, self.peer))
                            .map_err(|_| ENOTCONN)
                    } else if !socket.may_recv() {
                        Ok((0, self.peer))
                    } else {
                        Err(EAGAIN)
                    }
                } else {
                    let udp = network.sockets.get_mut::<udp::Socket>(self.handle);
                    let (packet, source) = udp.recv().map_err(|_| EAGAIN)?;
                    if self.peer.is_some_and(|peer| source.endpoint != peer) {
                        return Err(EAGAIN);
                    }
                    let size = packet.len().min(data.len());
                    data[..size].copy_from_slice(&packet[..size]);
                    Ok((size, Some(source.endpoint)))
                }
            })
            .ok_or(ENETDOWN)?;
            if !matches!(result, Err(EAGAIN))
                || self.nonblocking
                || dontwait
                || (self.read_timeout != 0 && ticks() >= until)
            {
                return result;
            }
            arch::wait_for_interrupt();
        }
    }

    pub fn events(&self, events: u16) -> u16 {
        with(|network| {
            network.poll();
            let (readable, writable, hung_up) = if self.tcp {
                let tcp = network.sockets.get::<tcp::Socket>(self.handle);
                (
                    tcp.can_recv()
                        || matches!(tcp.state(), tcp::State::Established | tcp::State::CloseWait)
                            && self.peer.is_none(),
                    tcp.can_send(),
                    self.peer.is_some() && !tcp.may_recv() && tcp.state() != tcp::State::SynSent,
                )
            } else {
                let udp = network.sockets.get::<udp::Socket>(self.handle);
                (udp.can_recv(), udp.can_send(), false)
            };
            (if readable { events & POLLIN } else { 0 })
                | (if writable { events & POLLOUT } else { 0 })
                | (if hung_up { POLLHUP } else { 0 })
        })
        .unwrap_or(POLLHUP)
    }

    pub fn shutdown(&self, how: u64) -> i64 {
        if !self.tcp || !matches!(how, 1 | 2) {
            return EINVAL;
        }
        with(|network| {
            network.sockets.get_mut::<tcp::Socket>(self.handle).close();
            network.poll();
            0
        })
        .unwrap_or(ENETDOWN)
    }
}

pub fn read_address(cr3: u64, address: u64) -> Result<IpEndpoint, i64> {
    let mut bytes = [0; size_of::<abi::Endpoint>()];
    if !memory::copy_in(cr3, address, &mut bytes) {
        return Err(EFAULT);
    }
    Ok(IpEndpoint::new(
        IpAddress::Ipv4(Ipv4Address::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        u16::from_ne_bytes([bytes[4], bytes[5]]),
    ))
}

pub fn write_address(cr3: u64, endpoint: IpEndpoint, address: u64) -> i64 {
    let mut bytes = [0; size_of::<abi::Endpoint>()];
    let IpAddress::Ipv4(ip) = endpoint.addr;
    bytes[..4].copy_from_slice(&ip.octets());
    bytes[4..].copy_from_slice(&endpoint.port.to_ne_bytes());
    if memory::copy_out(cr3, address, &bytes) {
        0
    } else {
        EFAULT
    }
}
