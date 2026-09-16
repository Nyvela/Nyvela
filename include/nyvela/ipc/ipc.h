#ifndef NYVIPC_H
#define NYVIPC_H

#include <stdint.h>

#define IPC_USERSPACE_ADDR 0x1F0

typedef enum ipc_status_t {
  IPC_SENT,
  IPC_UNREAD,
  IPC_RECEIVED,
} ipc_status_t;

typedef struct ipc_msg_t {
  ipc_status_t status;
  uint16_t target;
  uint8_t* msg;
  uint64_t size;
} ipc_msg_t;

void ipc_send(uint16_t target, uint8_t* msg, uint64_t size);

#endif // NYVIPC_H
