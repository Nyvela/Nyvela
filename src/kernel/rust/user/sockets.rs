use smoltcp::wire::IpEndpoint;

use crate::abi;

use super::{local_socket, memory, network};

#[derive(Clone, Copy)]
pub enum Socket {
    Inet(network::Socket),
    Local {
        handle: local_socket::Handle,
        nonblocking: bool,
        read_timeout: u64,
        write_timeout: u64,
    },
}

pub enum Address {
    Inet(IpEndpoint),
    Local(abi::LocalEndpoint),
}

impl Socket {
    pub fn open(kind: u64, flags: u64, uid: u32) -> Result<Self, i64> {
        if kind != abi::SOCKET_LOCAL_STREAM {
            return network::open(kind, flags).map(Self::Inet);
        }
        if flags & !abi::NONBLOCK != 0 {
            return Err(-22);
        }
        Ok(Self::local(local_socket::Handle::open(uid)?, flags != 0))
    }

    fn local(handle: local_socket::Handle, nonblocking: bool) -> Self {
        Self::Local {
            handle,
            nonblocking,
            read_timeout: 0,
            write_timeout: 0,
        }
    }

    pub fn close(self) {
        match self {
            Self::Inet(socket) => socket.close(),
            Self::Local { handle, .. } => handle.close(),
        }
    }

    pub fn is_stream(&self) -> bool {
        match self {
            Self::Inet(socket) => socket.is_stream(),
            Self::Local { .. } => true,
        }
    }

    pub fn nonblocking(&mut self) -> &mut bool {
        match self {
            Self::Inet(socket) => &mut socket.nonblocking,
            Self::Local { nonblocking, .. } => nonblocking,
        }
    }

    pub fn option(&mut self, option: u64, set: bool, value: u64) -> i64 {
        match (option, set) {
            (abi::SOCKET_KIND, false) => return if self.is_stream() { 1 } else { 2 },
            (abi::SOCKET_DOMAIN, false) => return i64::from(matches!(self, Self::Local { .. })),
            _ => {}
        }
        let (read, write) = match self {
            Self::Inet(socket) => match (option, set) {
                (abi::SOCKET_ERROR, false) => return i64::from(socket.pending_error()),
                (abi::SOCKET_REUSE, false) => return i64::from(socket.reuse_address),
                (abi::SOCKET_REUSE, true) => {
                    socket.reuse_address = value != 0;
                    return 0;
                }
                _ => (&mut socket.read_timeout, &mut socket.write_timeout),
            },
            Self::Local {
                read_timeout,
                write_timeout,
                ..
            } => match option {
                abi::SOCKET_ERROR if !set => return 0,
                abi::SOCKET_REUSE => return 0,
                _ => (read_timeout, write_timeout),
            },
        };
        let timer = match option {
            abi::SOCKET_READ_TIMEOUT => read,
            abi::SOCKET_WRITE_TIMEOUT => write,
            _ => return -92,
        };
        if set {
            *timer = value.min(i64::MAX as u64);
            0
        } else {
            *timer as i64
        }
    }

    pub fn set_address(&mut self, cr3: u64, address: u64, connect: bool) -> Result<(), i64> {
        match self {
            Self::Inet(socket) => {
                let address = network::read_address(cr3, address)?;
                let result = if connect {
                    socket.connect(address)
                } else {
                    socket.bind(address)
                };
                if result == 0 { Ok(()) } else { Err(result) }
            }
            Self::Local { handle, .. } => {
                let mut name = abi::LocalEndpoint::EMPTY;
                if !memory::copy_in(cr3, address, &mut name.name) {
                    return Err(-14);
                }
                let length = name
                    .name
                    .iter()
                    .position(|&byte| byte == 0)
                    .unwrap_or(name.name.len());
                if length == 0 {
                    return Err(-22);
                }
                name.name[length..].fill(0);
                if connect {
                    handle.connect(name)
                } else {
                    handle.bind(name)
                }
            }
        }
    }

    pub fn address(&self, peer: bool) -> Result<Address, i64> {
        match self {
            Self::Inet(socket) => socket.address(peer).map(Address::Inet),
            Self::Local { handle, .. } => handle.address(peer).map(Address::Local),
        }
    }

    pub fn listen(&mut self) -> i64 {
        match self {
            Self::Inet(socket) => socket.listen(),
            Self::Local { handle, .. } => handle.listen().err().unwrap_or(0),
        }
    }

    pub fn accept(&mut self, new_nonblocking: bool) -> Result<Self, i64> {
        match self {
            Self::Inet(socket) => socket.accept(new_nonblocking).map(Self::Inet),
            Self::Local {
                handle,
                nonblocking,
                read_timeout,
                ..
            } => handle
                .accept(*nonblocking, *read_timeout)
                .map(|handle| Self::local(handle, new_nonblocking)),
        }
    }

    pub fn shutdown(&self, how: u64) -> i64 {
        match self {
            Self::Inet(socket) => socket.shutdown(how),
            Self::Local { handle, .. } => handle.shutdown().err().unwrap_or(0),
        }
    }

    pub fn send(&mut self, data: &[u8], destination: Option<IpEndpoint>, dontwait: bool) -> i64 {
        match self {
            Self::Inet(socket) => socket.send(data, destination, dontwait),
            Self::Local {
                handle,
                nonblocking,
                write_timeout,
                ..
            } => {
                if destination.is_some() {
                    return -106;
                }
                handle
                    .send(data, dontwait || *nonblocking, *write_timeout)
                    .map_or_else(|error| error, |count| count as i64)
            }
        }
    }

    pub fn receive(
        &mut self,
        data: &mut [u8],
        dontwait: bool,
    ) -> Result<(usize, Option<Address>), i64> {
        match self {
            Self::Inet(socket) => socket
                .receive(data, dontwait)
                .map(|(size, address)| (size, address.map(Address::Inet))),
            Self::Local {
                handle,
                nonblocking,
                read_timeout,
                ..
            } => handle
                .receive(data, dontwait || *nonblocking, *read_timeout)
                .map(|size| (size, Some(Address::Local(abi::LocalEndpoint::EMPTY)))),
        }
    }

    pub fn events(&self, events: u16) -> u16 {
        match self {
            Self::Inet(socket) => socket.events(events),
            Self::Local { handle, .. } => handle.events(events),
        }
    }
}

impl Address {
    pub fn write(self, cr3: u64, address: u64) -> i64 {
        match self {
            Self::Inet(endpoint) => network::write_address(cr3, endpoint, address),
            Self::Local(endpoint) if memory::copy_out(cr3, address, &endpoint.name) => 0,
            Self::Local(_) => -14,
        }
    }
}
