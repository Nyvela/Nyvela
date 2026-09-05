#ifndef NYVIO_H
#define NYVIO_H

#include <stdint.h>

extern volatile uint8_t* VGA_MEM;
void kprints(const char *s, const uint8_t color);
void kprintln(const char *s, const uint8_t color);
void kprintnl();
void kprintinfo(const char *s, const uint8_t color);
void kprintsucc(const char *s, const uint8_t color);
void kprinterr(const char *s, const uint8_t color);
void kprintferr(const char *s, const uint8_t color);
void kprintinit(const char *s, const uint8_t color);

#endif // NYVIO_H
