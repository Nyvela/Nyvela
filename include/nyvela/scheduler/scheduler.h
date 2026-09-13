#ifndef NYVSCHEDULER_H
#define NYVSCHEDULER_H

#include "../../../include/nyvela/arch/x86_64/context.h"
#include "../../../include/nyvela/thread/thread.h"
#include <stdint.h>

#define SCHED_HZ 100
#define TIMER_DIVISOR 16

thread_t* scheduler_next();
void scheduler_tick(context_t* ctx);

#endif // NYVSCHEDULER_H
