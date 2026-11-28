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

uint8_t *kipc_alloc_payload(uint64_t size);
void kipc_free_message(const ipc_msg_t *msg);
// Takes ownership of the payload on success.
bool kipc_send(ipc_msg_t msg);
bool kipc_poll(ipc_msg_t* msg);

#endif // NYVIPC_H
