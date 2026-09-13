#ifndef NYV_FAULT_H
#define NYV_FAULT_H

#include <stdint.h>

// Debug fault dumper: prints vector, error code, fault RIP/CS/RFLAGS/RSP
// and 16 bytes at RIP in hex, then halts. Installed for #UD (6) and #DF (8)
// to diagnose user crashes; other vectors keep their existing stubs.
void fault_dump(uint64_t vec, uint64_t err, uint64_t has_err, uint64_t *frame);

#endif // NYV_FAULT_H
