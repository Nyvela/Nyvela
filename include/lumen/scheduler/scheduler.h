#ifndef NYVSCHEDULER_H
#define NYVSCHEDULER_H

#include "../../../include/lumen/arch/x86_64/context.h"
#include "../../../include/lumen/thread/thread.h"
#include <stdint.h>

#define SCHED_HZ 100
#define TIMER_DIVISOR 16

thread_t* scheduler_next(void);
void scheduler_tick(void);

#endif // NYVSCHEDULER_H
