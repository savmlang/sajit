#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif
  typedef struct AllocBlockSlice
  {
    uintptr_t rwview;
    uintptr_t rxview;
  } AllocBlockSlice;

  typedef struct AllocRequest
  {
    uintptr_t size;
    unsigned alignment;
  } AllocRequest;

  typedef struct SectionName
  {
    const char *ptr;
    size_t size;
  } SectionName;

  typedef void *(*getfn_ptr)(void *, const char *, size_t);
  typedef void (*offset_ptr)(void *, const char *, size_t, unsigned long long);

  typedef AllocBlockSlice (*allocate_t)(void *, AllocRequest, SectionName);

  typedef struct RustRTInterface
  {
    void *state;

    getfn_ptr getfnPtr;
    offset_ptr resolvefnOffset;
    allocate_t allocate;
  } RustRTInterface;

  int link_rtdyld(RustRTInterface *rt, const char *data, size_t size);

#ifdef __cplusplus
}
#endif