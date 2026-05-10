#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif

  typedef struct SizeAlignCS
  {
    size_t size;
    uint64_t align;
  } SizeAlignCS;

  typedef struct SizeAlignInfoCS
  {
    SizeAlignCS *ptr;
    size_t size;
  } SizeAlignInfoCS;

  typedef void (*sizealign_t)(void *state, SizeAlignInfoCS info);

  int dryrun_link(void *state, sizealign_t sizealigncall, const char *data, size_t size);

#ifdef __cplusplus
}
#endif
