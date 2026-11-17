#include "pd_fault_guard.h"

#include <stdatomic.h>
#include <setjmp.h>
#include <signal.h>

#ifdef _WIN32
#include <windows.h>
typedef jmp_buf pd_fault_jump;

static _Thread_local pd_fault_jump *activeJump;
static INIT_ONCE handlerOnce = INIT_ONCE_STATIC_INIT;
static PVOID handlerToken;

static int windowsFaultCode(DWORD code) {
  switch (code) {
  case EXCEPTION_ACCESS_VIOLATION:
  case EXCEPTION_IN_PAGE_ERROR:
  case EXCEPTION_ARRAY_BOUNDS_EXCEEDED:
  case EXCEPTION_DATATYPE_MISALIGNMENT:
    return SIGSEGV;
  case EXCEPTION_INT_DIVIDE_BY_ZERO:
  case EXCEPTION_FLT_DIVIDE_BY_ZERO:
  case EXCEPTION_FLT_INVALID_OPERATION:
  case EXCEPTION_FLT_OVERFLOW:
  case EXCEPTION_FLT_UNDERFLOW:
    return SIGFPE;
  case EXCEPTION_ILLEGAL_INSTRUCTION:
  case EXCEPTION_PRIV_INSTRUCTION:
    return SIGILL;
  default:
    return 0;
  }
}

static LONG CALLBACK faultHandler(PEXCEPTION_POINTERS exception) {
  int fault = windowsFaultCode(exception->ExceptionRecord->ExceptionCode);
  if (!fault || !activeJump)
    return EXCEPTION_CONTINUE_SEARCH;
  pd_fault_jump *jump = activeJump;
  activeJump = NULL;
  longjmp(*jump, fault);
}

static BOOL CALLBACK installFaultHandler(PINIT_ONCE once, PVOID parameter, PVOID *context) {
  (void)once;
  (void)parameter;
  (void)context;
  handlerToken = AddVectoredExceptionHandler(1, faultHandler);
  return handlerToken != NULL;
}

static int ensureFaultHandler(void) {
  return InitOnceExecuteOnce(&handlerOnce, installFaultHandler, NULL, NULL) && handlerToken != NULL;
}
#else

typedef sigjmp_buf pd_fault_jump;
static _Thread_local pd_fault_jump *activeJump;
static const int guardedSignals[] = {SIGSEGV, SIGBUS, SIGFPE, SIGILL};
static struct sigaction previousActions[sizeof(guardedSignals) / sizeof(guardedSignals[0])];
static struct sigaction guardAction;
static atomic_flag handlerLock = ATOMIC_FLAG_INIT;
static int handlerReady;

static void faultHandler(int signalNumber, siginfo_t *info, void *context) {
  if (activeJump) {
    pd_fault_jump *jump = activeJump;
    activeJump = NULL;
    siglongjmp(*jump, signalNumber);
  }
  for (unsigned i = 0; i < sizeof(guardedSignals) / sizeof(guardedSignals[0]); i++) {
    if (guardedSignals[i] != signalNumber)
      continue;
    const struct sigaction *previous = &previousActions[i];
    if ((previous->sa_flags & SA_SIGINFO) && previous->sa_sigaction) {
      previous->sa_sigaction(signalNumber, info, context);
    } else if (!(previous->sa_flags & SA_SIGINFO) && previous->sa_handler == SIG_IGN) {
      return;
    } else if (!(previous->sa_flags & SA_SIGINFO) && previous->sa_handler && previous->sa_handler != SIG_DFL) {
      previous->sa_handler(signalNumber);
    } else {
      sigaction(signalNumber, previous, NULL);
      raise(signalNumber);
    }
    return;
  }
}

static int ensureFaultHandler(void) {
  while (atomic_flag_test_and_set_explicit(&handlerLock, memory_order_acquire)) {
  }
  if (!handlerReady) {
    guardAction.sa_sigaction = faultHandler;
    guardAction.sa_flags = SA_SIGINFO;
#ifdef SA_ONSTACK
    guardAction.sa_flags |= SA_ONSTACK;
#endif
    sigemptyset(&guardAction.sa_mask);
    unsigned i = 0;
    for (; i < sizeof(guardedSignals) / sizeof(guardedSignals[0]); i++) {
      if (sigaction(guardedSignals[i], &guardAction, &previousActions[i]) != 0)
        break;
    }
    if (i == sizeof(guardedSignals) / sizeof(guardedSignals[0]))
      handlerReady = 1;
    else
      while (i > 0) {
        i--;
        sigaction(guardedSignals[i], &previousActions[i], NULL);
      }
  }
  int ready = handlerReady;
  atomic_flag_clear_explicit(&handlerLock, memory_order_release);
  return ready;
}
#endif

int pd_fault_guard_run(pd_fault_guard_call call, void *context, int *result, int *fault) {
  if (!call || !result || !fault || !ensureFaultHandler()) {
    if (fault)
      *fault = -1;
    return 0;
  }

  pd_fault_jump jump;
#ifdef _WIN32
  int raised = setjmp(jump);
#else
  int raised = sigsetjmp(jump, 1);
#endif
  if (raised) {
    activeJump = NULL;
    *fault = raised;
    return 0;
  }
  activeJump = &jump;
  *result = call(context);
  activeJump = NULL;
  *fault = 0;
  return 1;
}
