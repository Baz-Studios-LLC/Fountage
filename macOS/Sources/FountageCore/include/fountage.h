#ifndef FOUNTAGE_CORE_H
#define FOUNTAGE_CORE_H
#include <stdint.h>
#include <stddef.h>
char *fountage_analyze(const char *source, uint32_t format);
char *fountage_screenplay_output(const char *source);
char *fountage_screenplay_snapshot(const char *source);
char *fountage_scene_numbers_range(const char *source, uint32_t start, uint32_t remove, size_t location, size_t length);
char *fountage_scene_numbers(const char *source, uint32_t start, uint32_t remove);
char *fountage_outline(const char *source);
void fountage_string_free(char *value);
#endif
