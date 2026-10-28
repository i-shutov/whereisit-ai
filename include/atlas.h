#ifndef ATLAS_H
#define ATLAS_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
uint32_t atlas_abi_version(void);
/* Input borrowed UTF-8 NUL terminated. Output owned: release once below. */
char *atlas_call(const char *request_utf8);
void atlas_string_free(char *response);
#ifdef __cplusplus
}
#endif
#endif
