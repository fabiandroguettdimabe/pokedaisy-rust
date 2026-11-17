#pragma once

#include <fcntl.h>
#include <direct.h>
#include <io.h>
#include <stdlib.h>

#ifndef O_CLOEXEC
#define O_CLOEXEC 0
#endif

#ifndef _SC_PAGESIZE
#define _SC_PAGESIZE 30
extern "C" long sysconf(int name);
#endif

inline int mkdir(const char* path, int) { return _mkdir(path); }
inline char* realpath(const char* path, char* resolved) {
  return _fullpath(resolved, path, _MAX_PATH);
}
inline int fsync(int fd) { return _commit(fd); }
