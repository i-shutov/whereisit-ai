#ifndef ATLAS_FFI_H
#define ATLAS_FFI_H
#include <stdint.h>
char *atlas_call(const char *request_utf8);
void atlas_string_free(char *response_utf8);
uint32_t atlas_abi_version(void);
#endif
