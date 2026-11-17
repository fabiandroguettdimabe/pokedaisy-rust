#ifndef PD_FAULT_GUARD_H
#define PD_FAULT_GUARD_H

typedef int (*pd_fault_guard_call)(void *context);

/* Run a call while catching synchronous memory/CPU faults on the current thread. */
int pd_fault_guard_run(pd_fault_guard_call call, void *context, int *result, int *fault);

#endif
