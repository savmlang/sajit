#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif

  uint64_t calc(const char *data, size_t size);
  uint64_t calc_jitlink(void *symbpool, const char *data, size_t size);

#ifdef __cplusplus
}
#endif