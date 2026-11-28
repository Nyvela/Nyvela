#include "../../include/lumen/ipc/ipc.h"
#include "../../include/lumen/thread/thread.h"
#include "../../include/lumen/mm/heap.h"
#include "../../include/lumen/mm/pmm.h"
#include "../../include/lumen/syscall/syscall.h"
#include "../../include/lumen/lib/utils.h"

bool ipc_grow_queue(ipc_queue_t *queue) {
  uint64_t new_cap = queue->capacity * 2;
  ipc_msg_t *new_queue = kmalloc(new_cap * sizeof(ipc_msg_t));

  if (!new_queue) {
    return false;
  }

  for (uint64_t i = 0; i < queue->count; i++) {
    new_queue[i] = queue->queue[(queue->head + i) % queue->capacity];
  }

  kfree(queue->queue);

  queue->queue = new_queue;
  queue->capacity = new_cap;
  queue->head = 0;
  queue->tail = queue->count;

  return true;
}

uint8_t *kipc_alloc_payload(uint64_t size) {
  if (size <= SYSCALL_MAX_BUF) return kmalloc(size);
  return kpalloc_contiguous((size + 4095) / 4096);
}

void kipc_free_message(const ipc_msg_t *msg) {
  if (msg->size <= SYSCALL_MAX_BUF) {
    kfree(msg->msg);
  } else {
    kpfree_contiguous(msg->msg, (msg->size + 4095) / 4096);
  }
}

bool kipc_send(ipc_msg_t msg) {
  thread_t* thread = NULL;

  for (uint64_t i = 0; i < threads_length; i++) {
    if (threads[i]->tid == msg.target && threads[i]->state != THREAD_DEAD) {
      thread = threads[i];
      break;
    }
  }

  if (!thread) {
    return false;
  }

  ipc_queue_t* queue = thread->queue;

  if (!queue) {
    return false;
  }

  if (queue->count >= queue->capacity) {
    if (!ipc_grow_queue(queue)) {
      return false;
    }
  }

  queue->queue[queue->tail] = msg;
  queue->tail = (queue->tail + 1) % queue->capacity;
  queue->count++;

  return true;
}

bool kipc_poll(ipc_msg_t* out) {
  ipc_queue_t *queue = current_thread->queue;
  
  if (queue->count == 0) {
    return false;
  }
  
  *out = queue->queue[queue->head];
  queue->head = (queue->head + 1) % queue->capacity;
  queue->count--;

  return true;
}
