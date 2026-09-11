#ifndef NYVCONSOLE_H
#define NYVCONSOLE_H

#include <stdint.h>

void kprints(const char *s, const uint8_t color);
void kprintln(const char *s, const uint8_t color);
void kprintnl();
void kprintfail(const char *s, const uint8_t color);
void kprintinfo(const char *s, const uint8_t color);
void kprintsucc(const char *s, const uint8_t color);
void kprinterr(const char *s, const uint8_t color);
void kprintferr(const char *s, const uint8_t color);
void kprintinit(const char *s, const uint8_t color);

#endif // NYVCONSOLE_H
