/* GPL-3.0-or-later. Local engine ABI shared by the native Windows client. */
#ifndef LINGOMATE_ENGINE_H
#define LINGOMATE_ENGINE_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

enum LingoEngineStatus {
    LINGO_ENGINE_OK = 0,
    LINGO_ENGINE_INVALID_ARGUMENT = 1,
    LINGO_ENGINE_INVALID_UTF8 = 2,
    LINGO_ENGINE_INITIALIZATION_FAILED = 3,
    LINGO_ENGINE_PANIC_OR_POISONED = 4,
    LINGO_ENGINE_REQUEST_TOO_LARGE = 5,
    LINGO_ENGINE_REPLY_TOO_LARGE = 6,
    LINGO_ENGINE_INTERNAL_ERROR = 7
};

/* UTF-8 paths are at most 32768 bytes. memory_len == 0 disables memory.
 * Success returns an opaque handle. Failure resets *out_handle to NULL.
 * Error codes contain no underlying filesystem error text or user data. */
uint32_t LingoEngineCreate(const uint8_t *root, size_t root_len,
                          const uint8_t *memory, size_t memory_len,
                          void **out_handle);

/* Request <= 65536 bytes; reply <= 4194304 bytes, with no NUL or newline.
 * Protocol errors are existing JSON error responses with status OK.
 * Failure resets reply outputs to NULL/0. Requests on a live handle are
 * serialized. Caller buffers must remain valid for the whole call.
 * Every successful reply must be freed here, never in the caller's CRT. */
uint32_t LingoEngineRequest(void *handle, const uint8_t *request, size_t request_len,
                           uint8_t **out_bytes, size_t *out_len);
void LingoEngineFreeReply(uint8_t *bytes, size_t len);

/* Destroy once, only after all requests return and all replies are freed.
 * No request may overlap destruction or use the handle afterwards.
 * FreeReply and Destroy accept NULL as a no-op. */
void LingoEngineDestroy(void *handle);

#ifdef __cplusplus
}
#endif
#endif
