#include "../../include/nyvela/scheduler/scheduler.h"

volatile uint64_t ticks = 0;

void scheduler_tick(void) {
  ticks++;
}
