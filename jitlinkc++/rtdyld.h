#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif
  typedef struct AllocBlockSliceRTDYLD
  {
    uintptr_t rwview;
    uintptr_t rxview;
  } AllocBlockSliceRTDYLD;

  typedef struct AllocRequestRTDYLD
  {
    uintptr_t size;
    unsigned alignment;
  } AllocRequestRTDYLD;

  typedef struct SectionNameRTDYLD
  {
    const char *ptr;
    size_t size;
  } SectionNameRTDYLD;

  typedef void *(*getfn_ptr)(void *, const char *, size_t);
  typedef void (*offset_ptr)(void *, const char *, size_t, uint64_t);

  typedef AllocBlockSliceRTDYLD (*allocate_t_rtdyld)(void *, AllocRequestRTDYLD, SectionNameRTDYLD);

  typedef struct RustRTInterfaceRTDYLD
  {
    void *state;

    getfn_ptr getfnPtr;
    offset_ptr resolvefnOffset;
    allocate_t_rtdyld allocate;
  } RustRTInterfaceRTDYLD;

  int link_rtdyld(RustRTInterfaceRTDYLD *rt, const char *data, size_t size);

#ifdef __cplusplus
}
#endif