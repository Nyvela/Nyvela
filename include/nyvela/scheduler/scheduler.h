#ifndef NYVSCHEDULER_H
#define NYVSCHEDULER_H

#include <stdint.h>

#define SCHED_HZ 100
#define TIMER_DIVISOR 16

void scheduler_tick(void);

#endif // NYVSCHEDULER_H
