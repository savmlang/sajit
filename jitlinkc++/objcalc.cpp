#include "llvm/Object/ObjectFile.h"

using namespace llvm;
using namespace llvm::object;

#include <bit>
#include <cstdlib>
#include <span>

#include "jitlink.h"

inline void *align_to_64(void *ptr)
{
  auto int_ptr = std::bit_cast<uintptr_t>(ptr);
  int_ptr = (int_ptr + 63) & ~uintptr_t(63);
  return std::bit_cast<void *>(int_ptr);
}

extern "C"
{
  typedef struct State
  {
    void *alloc;
    int size;
  } State;

  void storeAddr(void *state, const char *data, uintptr_t len, uint64_t ptr)
  {
  }

  void freeJIT(void *state, AllocBlockSlicesJL slices)
  {
    delete[] slices.allocs;
  }

  uintptr_t getfn(void *state, const char *name, size_t len)
  {
    return 0;
  }

  AllocBlockSlicesJL allocjit(void *state, AllocRequestJL *req, size_t len)
  {
    AllocBlockSlicesJL data;

    auto vect = new AllocBlockSliceJL[len];

    std::span<AllocRequestJL> spanned(req, len);

    auto totalsize = 64;
    for (auto req : spanned)
    {
      totalsize += alignTo(req.size, std::max(req.alignment, (uint64_t)16));
    }

    auto alloc = malloc(totalsize);
    auto statedata = reinterpret_cast<State *>(state);
    statedata->alloc = alloc;
    statedata->size = totalsize;

    auto view = 0;
    auto alignedalloc = (uintptr_t)align_to_64(alloc);

    size_t i = 0;
    for (auto req : spanned)
    {
      AllocBlockSliceJL slice;

      slice.rxview = view;
      slice.rwview = alignedalloc;

      vect[i] = slice;

      auto toAdd = alignTo(req.size, std::max(req.alignment, (uint64_t)16));
      view += toAdd;
      alignedalloc += toAdd;

      i += 1;
    }

    data.allocs = vect;
    data.len = len;

    return data;
  }

  void err(void *state, const char *msg)
  {
    auto statedata = reinterpret_cast<State *>(state);
    statedata->size = 0;
  }

  uint64_t calc(const char *data, size_t size)
  {
    return (size * 110) / 100;
  }

  uint64_t calc_jitlink(void *symbpool, const char *data, size_t size)
  {
    RustMemoryInterfaceJL rinterface;

    State datastate{};
    rinterface.allocateJIT = allocjit;
    rinterface.state = &datastate;
    rinterface.getfnPtr = getfn;
    rinterface.freeJITStructure = freeJIT;
    rinterface.storeAddr = storeAddr;
    rinterface.onError = err;

    auto linker = create_linkctx(&rinterface);
    link_consume_linkctx(linker, symbpool, data, size);

    if (datastate.alloc)
    {
      free(datastate.alloc);
    }

    return datastate.size;
  }
}
