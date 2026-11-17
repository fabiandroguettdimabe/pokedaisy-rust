#pragma once

#include <stddef.h>

namespace vyntra_ds_windows {
void* aligned_alloc(size_t alignment, size_t size) noexcept;
void aligned_free(void* allocation) noexcept;
}  // namespace vyntra_ds_windows
