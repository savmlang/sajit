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

  typedef struct AllocBlockSlices
  {
    AllocBlockSlice *allocs;
    size_t len;
  } AllocBlockSlices;

  typedef struct AllocRequest
  {
    size_t size;
    uint64_t alignment;
  } AllocRequest;

  typedef uintptr_t (*ptr_t)(void *, const char *, size_t);

  /// All returned slices MUST refer to a single contiguous allocation.
  /// allocs[0].rxview is treated as the base address.
  typedef AllocBlockSlices (*alloc_t)(void *, AllocRequest *, size_t);

  /// This gives a temporary allocated C-Styled String
  /// For preservation, clone it into your addresspace
  typedef void (*error_cb_t)(void *, const char *msg);

  /// Frees the descriptor
  typedef void (*free_t)(void *, AllocBlockSlices);

  /// A method that stores the pointers happily
  typedef void (*addr_val)(void *, const char *, uintptr_t, uint64_t);

  /// This is an allocated interface (preferably stack allocated)
  /// that essentially serves as a VTable for data pointers.
  ///
  /// It must be alive until link_consume_linkctx
  typedef struct RustMemoryInterface
  {
    void *state;

    ptr_t getfnPtr;
    alloc_t allocateJIT;
    free_t freeJITStructure;
    error_cb_t onError;
    addr_val storeAddr;
  } RustMemoryInterface;

  /// Create a LinkerContext structure
  void *create_linkctx(RustMemoryInterface *linker);

  /// Free the LinkerContext structure.
  void free_linkctx(void *ctx_ptr);

  /// Creates a resuable symbolpool
  void *create_symbolpool();

  /// Frees the associated SymbolPool
  void free_symbolpool(void *symbolpool);

  /// Link using the linker created earlier. This automatically frees the ctx_ptr
  /// so free_linkctx is NOT TO BE CALLED
  ///
  /// NonZero int = FAILURE
  int link_consume_linkctx(void *ctx_ptr, void *symbol_pool, const char *data, size_t size);

#ifdef __cplusplus
}
#endif