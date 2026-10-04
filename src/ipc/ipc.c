#include "../../include/nyvela/ipc/ipc.h"
#include "../../include/nyvela/thread/thread.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/lib/utils.h"

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

bool kipc_send(uint64_t target, uint8_t* msg, uint64_t size) {
  thread_t* thread = NULL;

  for (uint64_t i = 0; i < threads_length; i++) {
    if (threads[i]->tid == target) {
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

  uint8_t *copied_msg = kmalloc(size);

  if (!copied_msg) {
    return false;
  }

  memcpy(copied_msg, msg, size);

  queue->queue[queue->tail] = (ipc_msg_t){
    .msg = copied_msg,
    .size = size,
    .target = target,
    .sender = current_thread->tid
  };

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
