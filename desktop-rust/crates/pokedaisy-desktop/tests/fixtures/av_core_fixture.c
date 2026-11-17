#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define EXPORT __declspec(dllexport)

typedef bool (*retro_environment_t)(unsigned, void *);
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

struct retro_geometry {
    unsigned base_width;
    unsigned base_height;
    unsigned max_width;
    unsigned max_height;
    float aspect_ratio;
};

struct retro_timing {
    double fps;
    double sample_rate;
};

struct retro_system_av_info {
    struct retro_geometry geometry;
    struct retro_timing timing;
};

static retro_environment_t environment_cb;
static retro_video_refresh_t video_cb;
static retro_audio_sample_t audio_cb;
static retro_audio_sample_batch_t audio_batch_cb;

EXPORT unsigned retro_api_version(void) { return 1; }
EXPORT void retro_set_environment(retro_environment_t callback) { environment_cb = callback; }
EXPORT void retro_set_video_refresh(retro_video_refresh_t callback) { video_cb = callback; }
EXPORT void retro_set_audio_sample(retro_audio_sample_t callback) { audio_cb = callback; }
EXPORT void retro_set_audio_sample_batch(retro_audio_sample_batch_t callback) { audio_batch_cb = callback; }
EXPORT void retro_set_input_poll(retro_input_poll_t callback) { (void)callback; }
EXPORT void retro_set_input_state(retro_input_state_t callback) { (void)callback; }
EXPORT void retro_init(void) {
    const unsigned pixel_format_rgb1555 = 0;
    if (environment_cb) environment_cb(10, (void *)&pixel_format_rgb1555);
}
EXPORT void retro_deinit(void) {}
EXPORT void retro_get_system_info(struct retro_system_info *info) {
    info->library_name = "Vyntra AV Fixture";
    info->library_version = "1.0";
    info->valid_extensions = "gba";
    info->need_fullpath = true;
    info->block_extract = false;
}
EXPORT bool retro_load_game(const struct retro_game_info *info) { return info && info->data && info->size; }
EXPORT void retro_unload_game(void) {}
EXPORT void retro_get_system_av_info(struct retro_system_av_info *info) {
    *info = (struct retro_system_av_info){
        .geometry = {2, 1, 2, 1, 2.0f},
        .timing = {60.0, 32000.0},
    };
}
EXPORT void retro_run(void) {
    static const uint16_t pixels[] = {0x001f, 0x7c00};
    static const int16_t samples[] = {1000, -1000, 2000, -2000};
    if (video_cb) video_cb(pixels, 2, 1, sizeof(pixels));
    if (audio_cb) audio_cb(3000, -3000);
    if (audio_batch_cb) audio_batch_cb(samples, 2);
    if (environment_cb) {
        struct retro_system_av_info updated = {
            .geometry = {4, 3, 8, 6, 4.0f / 3.0f},
            .timing = {50.0, 22050.0},
        };
        environment_cb(32, &updated);
    }
}
EXPORT size_t retro_serialize_size(void) { return 0; }
EXPORT bool retro_serialize(void *data, size_t size) { (void)data; (void)size; return false; }
EXPORT bool retro_unserialize(const void *data, size_t size) { (void)data; (void)size; return false; }
EXPORT void *retro_get_memory_data(unsigned id) { (void)id; return NULL; }
EXPORT size_t retro_get_memory_size(unsigned id) { (void)id; return 0; }
EXPORT void retro_set_controller_port_device(unsigned port, unsigned device) { (void)port; (void)device; }
