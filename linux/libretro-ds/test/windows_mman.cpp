#include <sys/mman.h>

#include <array>
#include <cassert>
#include <cstdint>
#include <filesystem>
#include <fstream>

#include <fcntl.h>
#include <io.h>

#include <windows_compat/aligned_alloc.h>

int main() {
  auto* aligned = static_cast<std::uint8_t*>(vyntra_ds_windows::aligned_alloc(2048, 4096));
  assert(aligned != nullptr);
  assert(reinterpret_cast<std::uintptr_t>(aligned) % 2048 == 0);
  aligned[0] = 0x3c;
  assert(aligned[0] == 0x3c);
  vyntra_ds_windows::aligned_free(aligned);

  constexpr size_t kBytes = 8192;
  auto* anonymous = static_cast<std::uint8_t*>(
      mmap(nullptr, kBytes, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0));
  assert(anonymous != MAP_FAILED);
  anonymous[0] = 0x5a;
  anonymous[kBytes - 1] = 0xa5;
  assert(anonymous[0] == 0x5a && anonymous[kBytes - 1] == 0xa5);
  assert(munmap(anonymous, kBytes) == 0);

  const auto path = std::filesystem::temp_directory_path() / "ds-windows-mman-test.bin";
  std::array<std::uint8_t, 131072> bytes{};
  for (size_t i = 0; i < bytes.size(); ++i) bytes[i] = static_cast<std::uint8_t>(i % 251);
  {
    std::ofstream file(path, std::ios::binary | std::ios::trunc);
    file.write(reinterpret_cast<const char*>(bytes.data()), bytes.size());
    assert(file.good());
  }
  const int fd = _open(path.string().c_str(), _O_RDONLY | _O_BINARY);
  assert(fd >= 0);
  auto* mapped = static_cast<const std::uint8_t*>(
      mmap(nullptr, kBytes, PROT_READ, MAP_PRIVATE, fd, 4096));
  _close(fd);
  assert(mapped != MAP_FAILED);
  assert(mapped[0] == bytes[4096]);
  assert(mapped[kBytes - 1] == bytes[4096 + kBytes - 1]);
  assert(munmap(const_cast<std::uint8_t*>(mapped), kBytes) == 0);
  std::filesystem::remove(path);
}
