#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif
  typedef struct AllocBlockSliceJL
  {
    uintptr_t rwview;
    uintptr_t rxview;
  } AllocBlockSliceJL;

  typedef struct AllocBlockSlicesJL
  {
    AllocBlockSliceJL *allocs;
    size_t len;
  } AllocBlockSlicesJL;

  typedef struct AllocRequestJL
  {
    size_t size;
    uint64_t alignment;
  } AllocRequestJL;

  typedef uintptr_t (*ptr_t_jl)(void *, const char *, size_t);

  /// All returned slices MUST refer to a single contiguous allocation.
  /// allocs[0].rxview is treated as the base address.
  typedef AllocBlockSlicesJL (*alloc_t_jl)(void *, AllocRequestJL *, size_t);

  /// This gives a temporary allocated C-Styled String
  /// For preservation, clone it into your addresspace
  typedef void (*error_cb_tjl)(void *, const char *msg);

  /// Frees the descriptor
  typedef void (*free_t_jl)(void *, AllocBlockSlicesJL);

  /// A method that stores the pointers happily
  typedef void (*addr_val_jl)(void *, const char *, uintptr_t, uint64_t);

  /// This is an allocated interface (preferably stack allocated)
  /// that essentially serves as a VTable for data pointers.
  ///
  /// It must be alive until link_consume_linkctx
  typedef struct RustMemoryInterfaceJL
  {
    void *state;

    ptr_t_jl getfnPtr;
    alloc_t_jl allocateJIT;
    free_t_jl freeJITStructure;
    error_cb_tjl onError;
    addr_val_jl storeAddr;
  } RustMemoryInterfaceJL;

  /// Create a LinkerContext structure
  void *create_linkctx(RustMemoryInterfaceJL *linker);

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