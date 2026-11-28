//! Account-scoped byte streams. Access is serialized by the boot CPU.
use crate::abi::LocalEndpoint;

use super::{arch, syscalls::ticks};

const CAPACITY: usize = 32;
const BUFFER_SIZE: usize = 8192;
const BACKLOG: usize = 8;
const AGAIN: i64 = -11;
const NOT_CONNECTED: i64 = -107;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Handle(usize);

struct Connection {
    peer: Option<Handle>,
    pending: Option<(Handle, u64)>,
    name: LocalEndpoint,
    server_side: bool,
    write_closed: bool,
    peer_write_closed: bool,
}

enum State {
    Fresh,
    Bound(LocalEndpoint),
    Listening { name: LocalEndpoint, sequence: u64 },
    Connected(Connection),
}

struct Slot {
    uid: u32,
    state: State,
    head: usize,
    length: usize,
}

impl Slot {
    const EMPTY: Self = Self {
        uid: 0,
        state: State::Fresh,
        head: 0,
        length: 0,
    };
}

static mut BUFFERS: [[u8; BUFFER_SIZE]; CAPACITY] = [[0; BUFFER_SIZE]; CAPACITY];
static mut SLOTS: [Option<Slot>; CAPACITY] = [const { None }; CAPACITY];

// The reference stays within each operation, including each iteration of a
// blocking operation. Interrupts may schedule a peer between iterations.
fn with<T>(operation: impl FnOnce(&mut [Option<Slot>; CAPACITY]) -> T) -> T {
    unsafe { operation(&mut *(&raw mut SLOTS)) }
}

fn free_slot(slots: &[Option<Slot>; CAPACITY]) -> Result<Handle, i64> {
    slots
        .iter()
        .position(Option::is_none)
        .map(Handle)
        .ok_or(-23)
}

fn wait<T>(
    nonblocking: bool,
    timeout: u64,
    mut operation: impl FnMut() -> Result<T, i64>,
) -> Result<T, i64> {
    let deadline = ticks().saturating_add(timeout);
    loop {
        match operation() {
            Err(AGAIN) if !nonblocking && (timeout == 0 || ticks() < deadline) => {
                arch::wait_for_interrupt();
            }
            result => return result,
        }
    }
}

impl Handle {
    pub fn open(uid: u32) -> Result<Self, i64> {
        with(|slots| {
            let handle = free_slot(slots)?;
            slots[handle.0] = Some(Slot { uid, ..Slot::EMPTY });
            Ok(handle)
        })
    }

    pub fn bind(self, name: LocalEndpoint) -> Result<(), i64> {
        with(|slots| {
            let slot = slots[self.0].as_ref().unwrap();
            if !matches!(slot.state, State::Fresh) {
                return Err(-22);
            }
            if slots.iter().flatten().any(|other| {
                other.uid == slot.uid
                    && matches!(other.state, State::Bound(bound) | State::Listening { name: bound, .. } if bound == name)
            }) {
                return Err(-98);
            }
            slots[self.0].as_mut().unwrap().state = State::Bound(name);
            Ok(())
        })
    }

    pub fn listen(self) -> Result<(), i64> {
        with(|slots| {
            let state = &mut slots[self.0].as_mut().unwrap().state;
            match *state {
                State::Bound(name) => *state = State::Listening { name, sequence: 0 },
                State::Listening { .. } => {}
                _ => return Err(-22),
            }
            Ok(())
        })
    }

    pub fn connect(self, name: LocalEndpoint) -> Result<(), i64> {
        with(|slots| {
            let slot = slots[self.0].as_ref().unwrap();
            if !matches!(slot.state, State::Fresh) {
                return Err(-106);
            }
            let uid = slot.uid;
            let listener = slots
                .iter()
                .position(|slot| {
                    slot.as_ref().is_some_and(|slot| {
                        slot.uid == uid
                            && matches!(slot.state, State::Listening { name: bound, .. } if bound == name)
                    })
                })
                .map(Handle)
                .ok_or(-111)?;
            if pending(slots, listener).count() >= BACKLOG {
                return Err(AGAIN);
            }
            let child = free_slot(slots)?;
            let State::Listening { sequence, .. } = &mut slots[listener.0].as_mut().unwrap().state
            else {
                unreachable!()
            };
            let position = *sequence;
            *sequence += 1;
            let connected = |peer, pending, server_side| {
                State::Connected(Connection {
                    peer: Some(peer),
                    pending,
                    name,
                    server_side,
                    write_closed: false,
                    peer_write_closed: false,
                })
            };
            slots[child.0] = Some(Slot {
                uid,
                state: connected(self, Some((listener, position)), true),
                ..Slot::EMPTY
            });
            slots[self.0].as_mut().unwrap().state = connected(child, None, false);
            Ok(())
        })
    }

    pub fn accept(self, nonblocking: bool, timeout: u64) -> Result<Self, i64> {
        wait(nonblocking, timeout, || {
            with(|slots| {
                if !matches!(
                    slots[self.0].as_ref().unwrap().state,
                    State::Listening { .. }
                ) {
                    return Err(-22);
                }
                let (child, _) = pending(slots, self)
                    .min_by_key(|&(_, sequence)| sequence)
                    .ok_or(AGAIN)?;
                connection(&mut slots[child.0].as_mut().unwrap().state)?.pending = None;
                Ok(child)
            })
        })
    }

    pub fn send(self, bytes: &[u8], nonblocking: bool, timeout: u64) -> Result<usize, i64> {
        wait(nonblocking, timeout, || {
            with(|slots| {
                let connection = connection(&mut slots[self.0].as_mut().unwrap().state)?;
                let peer = connection
                    .peer
                    .filter(|_| !connection.write_closed)
                    .ok_or(-32)?;
                let slot = slots[peer.0].as_mut().unwrap();
                let count = bytes.len().min(BUFFER_SIZE - slot.length);
                if count == 0 && !bytes.is_empty() {
                    return Err(AGAIN);
                }
                let start = (slot.head + slot.length) % BUFFER_SIZE;
                let first = count.min(BUFFER_SIZE - start);
                let buffer = unsafe { &mut (*(&raw mut BUFFERS))[peer.0] };
                buffer[start..start + first].copy_from_slice(&bytes[..first]);
                buffer[..count - first].copy_from_slice(&bytes[first..count]);
                slot.length += count;
                Ok(count)
            })
        })
    }

    pub fn receive(self, bytes: &mut [u8], nonblocking: bool, timeout: u64) -> Result<usize, i64> {
        wait(nonblocking, timeout, || {
            with(|slots| {
                let slot = slots[self.0].as_mut().unwrap();
                let connection = connection(&mut slot.state)?;
                let count = bytes.len().min(slot.length);
                if count == 0 && !bytes.is_empty() && !connection.peer_write_closed {
                    return Err(AGAIN);
                }
                let first = count.min(BUFFER_SIZE - slot.head);
                let buffer = unsafe { &(*(&raw const BUFFERS))[self.0] };
                bytes[..first].copy_from_slice(&buffer[slot.head..slot.head + first]);
                bytes[first..count].copy_from_slice(&buffer[..count - first]);
                slot.head = (slot.head + count) % BUFFER_SIZE;
                slot.length -= count;
                Ok(count)
            })
        })
    }

    pub fn events(self, events: u16) -> u16 {
        with(|slots| {
            let slot = slots[self.0].as_ref().unwrap();
            match &slot.state {
                State::Listening { .. } if pending(slots, self).next().is_some() => events & 1,
                State::Connected(connection) => {
                    let readable = slot.length != 0 || connection.peer_write_closed;
                    let writable = !connection.write_closed
                        && connection.peer.is_some_and(|peer| {
                            slots[peer.0].as_ref().unwrap().length < BUFFER_SIZE
                        });
                    (u16::from(readable) | (u16::from(writable) << 2)) & events
                        | if connection.peer.is_none() { 16 } else { 0 }
                }
                _ => 0,
            }
        })
    }

    pub fn address(self, peer: bool) -> Result<LocalEndpoint, i64> {
        with(|slots| {
            let slot = slots[self.0].as_ref().unwrap();
            match &slot.state {
                State::Bound(name) | State::Listening { name, .. } if !peer => Ok(*name),
                State::Connected(connection) if peer != connection.server_side => {
                    Ok(connection.name)
                }
                State::Connected(_) => Ok(LocalEndpoint::EMPTY),
                _ if !peer => Ok(LocalEndpoint::EMPTY),
                _ => Err(NOT_CONNECTED),
            }
        })
    }

    pub fn shutdown(self) -> Result<(), i64> {
        with(|slots| {
            let endpoint = connection(&mut slots[self.0].as_mut().unwrap().state)?;
            endpoint.write_closed = true;
            if let Some(peer) = endpoint.peer {
                connection(&mut slots[peer.0].as_mut().unwrap().state)?.peer_write_closed = true;
            }
            Ok(())
        })
    }

    pub fn close(self) {
        with(|slots| {
            loop {
                let child = pending(slots, self).next();
                let Some((child, _)) = child else { break };
                close(slots, child);
            }
            close(slots, self);
        });
    }
}

fn connection(state: &mut State) -> Result<&mut Connection, i64> {
    match state {
        State::Connected(connection) => Ok(connection),
        _ => Err(NOT_CONNECTED),
    }
}

fn pending(
    slots: &[Option<Slot>; CAPACITY],
    listener: Handle,
) -> impl Iterator<Item = (Handle, u64)> {
    slots.iter().enumerate().filter_map(move |(index, slot)| {
        let State::Connected(connection) = &slot.as_ref()?.state else {
            return None;
        };
        let (owner, sequence) = connection.pending?;
        (owner == listener).then_some((Handle(index), sequence))
    })
}

fn close(slots: &mut [Option<Slot>; CAPACITY], handle: Handle) {
    let peer = connection(&mut slots[handle.0].as_mut().unwrap().state)
        .ok()
        .and_then(|connection| connection.peer);
    if let Some(peer) = peer {
        let other = connection(&mut slots[peer.0].as_mut().unwrap().state).unwrap();
        other.peer = None;
        other.peer_write_closed = true;
    }
    slots[handle.0] = None;
}
