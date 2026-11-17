#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define EXPORT __declspec(dllexport)

#ifdef PROBE_MISSING_DEPENDENCY
__declspec(dllimport) void vyntra_probe_dependency(void);
#endif
#ifdef PROBE_CRASH
#include <stdlib.h>
#endif

typedef void (*retro_environment_t)(unsigned, void *);
typedef void (*retro_video_refresh_t)(const void *, unsigned, unsigned, size_t);
typedef void (*retro_audio_sample_t)(int16_t, int16_t);
typedef size_t (*retro_audio_sample_batch_t)(const int16_t *, size_t);
typedef void (*retro_input_poll_t)(void);
typedef int16_t (*retro_input_state_t)(unsigned, unsigned, unsigned, unsigned);

struct retro_system_info {
    const char *library_name;
    const char *library_version;
    const char *valid_extensions;
    bool need_fullpath;
    bool block_extract;
};

struct retro_game_info {
    const char *path;
    const void *data;
    size_t size;
    const char *meta;
};

EXPORT unsigned retro_api_version(void) { return 1; }
EXPORT void retro_set_environment(retro_environment_t callback) { (void)callback; }
EXPORT void retro_set_video_refresh(retro_video_refresh_t callback) { (void)callback; }
EXPORT void retro_set_audio_sample(retro_audio_sample_t callback) { (void)callback; }
EXPORT void retro_set_audio_sample_batch(retro_audio_sample_batch_t callback) { (void)callback; }
EXPORT void retro_set_input_poll(retro_input_poll_t callback) { (void)callback; }
EXPORT void retro_set_input_state(retro_input_state_t callback) { (void)callback; }
EXPORT void retro_init(void) {
#ifdef PROBE_HANG
    for (;;) {}
#endif
#ifdef PROBE_CRASH
    abort();
#endif
#ifdef PROBE_MISSING_DEPENDENCY
    vyntra_probe_dependency();
#endif
}
EXPORT void retro_deinit(void) {}
EXPORT void retro_get_system_info(struct retro_system_info *info) {
    info->library_name = "Vyntra Probe Fixture";
    info->library_version = "1.0";
    info->valid_extensions = "gba";
    info->need_fullpath = true;
    info->block_extract = false;
}
EXPORT bool retro_load_game(const struct retro_game_info *info) { (void)info; return true; }
EXPORT void retro_unload_game(void) {}
EXPORT void retro_run(void) {}
EXPORT void retro_get_system_av_info(void *info) { (void)info; }
EXPORT size_t retro_serialize_size(void) { return 0; }
EXPORT bool retro_serialize(void *data, size_t size) { (void)data; (void)size; return false; }
EXPORT bool retro_unserialize(const void *data, size_t size) { (void)data; (void)size; return false; }
EXPORT void *retro_get_memory_data(unsigned id) { (void)id; return NULL; }
EXPORT size_t retro_get_memory_size(unsigned id) { (void)id; return 0; }
EXPORT void retro_set_controller_port_device(unsigned port, unsigned device) { (void)port; (void)device; }
