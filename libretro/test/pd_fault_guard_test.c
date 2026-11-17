#include "../pd_fault_guard.h"

#include <stdio.h>

static int normal_call(void *context) { return *(const int *)context + 1; }

static int invalid_read(void *context) {
  (void)context;
  return *(volatile const int *)0;
}

int main(void) {
  int value = 41;
  int fault = 0;
  int result = 0;
  if (!pd_fault_guard_run(normal_call, &value, &result, &fault) || result != 42 || fault != 0) {
    fputs("normal guarded call did not return its result\n", stderr);
    return 1;
  }
  if (pd_fault_guard_run(invalid_read, NULL, &result, &fault) || fault == 0) {
    fputs("invalid memory read was not caught\n", stderr);
    return 1;
  }
  return 0;
}
