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

struct retro_memory_descriptor {
    uint64_t flags;
    void *ptr;
    size_t offset;
    size_t start;
    size_t select;
    size_t disconnect;
    size_t len;
    const char *addrspace;
};

struct retro_memory_map {
    const struct retro_memory_descriptor *descriptors;
    unsigned num_descriptors;
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
static retro_input_poll_t input_poll_cb;
static retro_input_state_t input_state_cb;
static uint8_t system_ram[4] = {0, 2, 3, 4};
static uint8_t ewram[0x2000] = {
    [0] = 1, [1] = 2, [2] = 3, [3] = 4, [4] = 5, [5] = 6,
    [0x100] = 1,
    [0x101c] = 73,
    [0x1020] = 25,
    [0x102c] = 33,
    [0x1034] = 15,
    [0x1054] = 12,
    [0x1056] = 5,
    [0x1058] = 10,
};
static uint8_t iwram[8] = {7, 8, 9, 10};
static uint8_t mirrored_ram[0x200] = {
    [0xff] = 0xaa,
    [0x100] = 0xbb,
};
static uint8_t bios_space_ram[4] = {0xb1, 0xb2, 0xb3, 0xb4};
static uint8_t wram_space_ram[4] = {0xc1, 0xc2, 0xc3, 0xc4};
static uint8_t selected_ram[0x200] = {[0xff] = 0xd1, [0x100] = 0xd2};
static const struct retro_memory_descriptor regions[] = {
    { .flags = 1u << 2, .ptr = ewram, .start = 0x02000000, .select = 0xff000000, .len = sizeof(ewram) },
    { .flags = 1u << 2, .ptr = iwram, .start = 0x03000000, .select = 0xff000000, .len = sizeof(iwram) },
    { .flags = 1u << 2, .ptr = mirrored_ram, .start = 0x04000000, .select = 0xff000000, .disconnect = 0x100, .len = sizeof(mirrored_ram) },
    { .flags = 1u << 2, .ptr = bios_space_ram, .start = 0x05000000, .select = 0xff000000, .len = sizeof(bios_space_ram), .addrspace = "BIOS" },
    { .flags = 1u << 2, .ptr = wram_space_ram, .start = 0x05000000, .select = 0xff000000, .len = sizeof(wram_space_ram), .addrspace = "WRAM" },
    { .flags = 1u << 2, .ptr = selected_ram, .start = 0x06000000, .select = 0xff000100, .len = sizeof(selected_ram) },
};
static const struct retro_memory_map map = { regions, 6 };

EXPORT unsigned retro_api_version(void) { return 1; }
EXPORT void retro_set_environment(retro_environment_t callback) { environment_cb = callback; }
EXPORT void retro_set_video_refresh(retro_video_refresh_t callback) { video_cb = callback; }
EXPORT void retro_set_audio_sample(retro_audio_sample_t callback) { audio_cb = callback; }
EXPORT void retro_set_audio_sample_batch(retro_audio_sample_batch_t callback) { audio_batch_cb = callback; }
EXPORT void retro_set_input_poll(retro_input_poll_t callback) { input_poll_cb = callback; }
EXPORT void retro_set_input_state(retro_input_state_t callback) { input_state_cb = callback; }
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
EXPORT bool retro_load_game(const struct retro_game_info *info) {
    if (environment_cb) environment_cb(65572, (void *)&map);
    return info && info->data && info->size;
}
EXPORT void retro_unload_game(void) {}
EXPORT void retro_get_system_av_info(struct retro_system_av_info *info) {
    *info = (struct retro_system_av_info){
        .geometry = {2, 1, 2, 1, 2.0f},
        .timing = {60.0, 32000.0},
    };
}
EXPORT void retro_run(void) {
    system_ram[0]++;
    static uint16_t pixels[2];
    static const int16_t samples[] = {1000, -1000, 2000, -2000};
    if (input_poll_cb) input_poll_cb();
    pixels[0] = input_state_cb && input_state_cb(0, 1, 0, 8) ? 0x03e0 : 0x001f;
    pixels[1] = 0x7c00;
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
EXPORT void *retro_get_memory_data(unsigned id) { return id == 2 ? system_ram : NULL; }
EXPORT size_t retro_get_memory_size(unsigned id) { return id == 2 ? sizeof(system_ram) : 0; }
EXPORT void retro_set_controller_port_device(unsigned port, unsigned device) { (void)port; (void)device; }
