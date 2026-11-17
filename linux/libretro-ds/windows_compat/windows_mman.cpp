#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <io.h>
#include <malloc.h>

#include <cerrno>
#include <cstdint>
#include <map>
#include <mutex>
#include <sys/mman.h>

#include <windows_compat/aligned_alloc.h>
#include "posix_compat.h"

namespace {

struct Mapping {
  void* base;
  size_t mapped_length;
  HANDLE section;
  bool file_backed;
};

std::map<void*, Mapping>& mappings() {
  static std::map<void*, Mapping> value;
  return value;
}

std::mutex& mappings_mutex() {
  static std::mutex value;
  return value;
}

DWORD page_protection(int protection, bool file_backed) {
  if ((protection & PROT_EXEC) != 0) {
    if ((protection & PROT_WRITE) != 0) return file_backed ? PAGE_EXECUTE_WRITECOPY : PAGE_EXECUTE_READWRITE;
    if ((protection & PROT_READ) != 0) return PAGE_EXECUTE_READ;
    return PAGE_EXECUTE;
  }
  if ((protection & PROT_WRITE) != 0) return file_backed ? PAGE_WRITECOPY : PAGE_READWRITE;
  if ((protection & PROT_READ) != 0) return PAGE_READONLY;
  return PAGE_NOACCESS;
}

DWORD view_access(int protection, bool shared) {
  DWORD access = 0;
  if ((protection & PROT_READ) != 0) access |= FILE_MAP_READ;
  if ((protection & PROT_WRITE) != 0) access |= shared ? FILE_MAP_WRITE : FILE_MAP_COPY;
  if ((protection & PROT_EXEC) != 0) access |= FILE_MAP_EXECUTE;
  if (access == 0) access = FILE_MAP_READ;
  return access;
}

void fail(int error) {
  errno = error;
  SetLastError(ERROR_SUCCESS);
}

}  // namespace

void* vyntra_ds_windows::aligned_alloc(size_t alignment, size_t size) noexcept {
  return _aligned_malloc(size, alignment);
}

void vyntra_ds_windows::aligned_free(void* allocation) noexcept {
  _aligned_free(allocation);
}

extern "C" void* mmap(void* address, size_t length, int protection, int flags, int fd,
                       off_t offset) {
  if (length == 0 || offset < 0) {
    fail(EINVAL);
    return MAP_FAILED;
  }

  const bool anonymous = (flags & MAP_ANONYMOUS) != 0;
  const bool fixed = (flags & MAP_FIXED) != 0;
  const bool shared = (flags & MAP_SHARED) != 0;
  if (!anonymous && fd < 0) {
    fail(EBADF);
    return MAP_FAILED;
  }

  if (anonymous) {
    const DWORD allocation = MEM_RESERVE | ((protection == PROT_NONE) ? 0 : MEM_COMMIT);
    void* base = VirtualAlloc(address, length, allocation, page_protection(protection, false));
    if (!base || (fixed && base != address)) {
      if (base) VirtualFree(base, 0, MEM_RELEASE);
      fail(GetLastError() == ERROR_NOT_ENOUGH_MEMORY ? ENOMEM : EINVAL);
      return MAP_FAILED;
    }
    try {
      std::lock_guard<std::mutex> lock(mappings_mutex());
      mappings().emplace(base, Mapping{base, length, nullptr, false});
    } catch (...) {
      VirtualFree(base, 0, MEM_RELEASE);
      fail(ENOMEM);
      return MAP_FAILED;
    }
    return base;
  }

  const DWORD granularity = [] {
    SYSTEM_INFO info{};
    GetSystemInfo(&info);
    return info.dwAllocationGranularity;
  }();
  const auto unsigned_offset = static_cast<std::uint64_t>(offset);
  const std::uint64_t aligned_offset = unsigned_offset - (unsigned_offset % granularity);
  const size_t delta = static_cast<size_t>(unsigned_offset - aligned_offset);
  if (length > SIZE_MAX - delta) {
    fail(EOVERFLOW);
    return MAP_FAILED;
  }
  const size_t mapped_length = length + delta;
  const intptr_t native_fd = _get_osfhandle(fd);
  if (native_fd == -1) {
    fail(EBADF);
    return MAP_FAILED;
  }
  HANDLE file = reinterpret_cast<HANDLE>(native_fd);
  const DWORD section_protection = page_protection(protection, true);
  HANDLE section = CreateFileMappingW(file, nullptr, section_protection, 0, 0, nullptr);
  if (!section) {
    fail(GetLastError() == ERROR_NOT_ENOUGH_MEMORY ? ENOMEM : EACCES);
    return MAP_FAILED;
  }

  const std::uint64_t view_offset = aligned_offset;
  void* base = MapViewOfFileEx(section, view_access(protection, shared),
                               static_cast<DWORD>(view_offset >> 32),
                               static_cast<DWORD>(view_offset & 0xffffffffu), mapped_length,
                               fixed ? address : nullptr);
  if (!base || (fixed && base != address)) {
    if (base) UnmapViewOfFile(base);
    CloseHandle(section);
    fail(GetLastError() == ERROR_NOT_ENOUGH_MEMORY ? ENOMEM : EINVAL);
    return MAP_FAILED;
  }
  void* result = static_cast<std::uint8_t*>(base) + delta;
  try {
    std::lock_guard<std::mutex> lock(mappings_mutex());
    mappings().emplace(result, Mapping{base, mapped_length, section, true});
  } catch (...) {
    UnmapViewOfFile(base);
    CloseHandle(section);
    fail(ENOMEM);
    return MAP_FAILED;
  }
  return result;
}

extern "C" int munmap(void* address, size_t) {
  Mapping mapping{};
  {
    std::lock_guard<std::mutex> lock(mappings_mutex());
    const auto found = mappings().find(address);
    if (found == mappings().end()) {
      fail(EINVAL);
      return -1;
    }
    mapping = found->second;
    mappings().erase(found);
  }
  const BOOL released = mapping.file_backed ? UnmapViewOfFile(mapping.base)
                                           : VirtualFree(mapping.base, 0, MEM_RELEASE);
  if (mapping.section) CloseHandle(mapping.section);
  if (!released) {
    fail(EINVAL);
    return -1;
  }
  return 0;
}

extern "C" int mprotect(void* address, size_t length, int protection) {
  DWORD old_protection = 0;
  if (!VirtualProtect(address, length, page_protection(protection, false), &old_protection)) {
    fail(EACCES);
    return -1;
  }
  return 0;
}

extern "C" long sysconf(int name) {
  if (name != _SC_PAGESIZE) {
    fail(EINVAL);
    return -1;
  }
  SYSTEM_INFO info{};
  GetSystemInfo(&info);
  return static_cast<long>(info.dwPageSize);
}
