#include "../../include/nyvela/ipc/ipc.h"

// only for userspace code
void ipc_send(uint16_t target, uint8_t* msg, uint64_t size) {
  ipc_msg_t *addr = (ipc_msg_t*)IPC_USERSPACE_ADDR;

  *addr = (ipc_msg_t){
    .status = IPC_SENT,
    .msg = msg,
    .target = target,
    .size = size
  };
}
