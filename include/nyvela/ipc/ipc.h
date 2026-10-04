#ifndef NYVIPC_H
#define NYVIPC_H

#include <stdint.h>

typedef struct ipc_msg {
  uint64_t target;
  uint64_t sender;
  uint8_t* msg;
  uint64_t size;
} ipc_msg_t;

typedef struct ipc_queue {
  ipc_msg_t* queue;
  uint64_t capacity, head, tail, count;
} ipc_queue_t;

bool kipc_send(uint64_t target, uint8_t* msg, uint64_t size);
bool kipc_poll(ipc_msg_t* msg);

#endif // NYVIPC_H
