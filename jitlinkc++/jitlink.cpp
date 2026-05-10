#include "llvm/ExecutionEngine/JITLink/JITLink.h"
#include "llvm/Support/MemoryBuffer.h"
#include <span>

using namespace llvm;
using namespace llvm::jitlink;

extern "C"
{
  struct AllocBlockSliceJL
  {
    uintptr_t rwview;
    uintptr_t rxview;
  };

  struct AllocBlockSlicesJL
  {
    AllocBlockSliceJL *allocs;
    size_t len;
  };

  struct AllocRequestJL
  {
    size_t size;
    uint64_t alignment;
  };

  typedef uintptr_t (*ptr_t)(void *, const char *, size_t);
  typedef AllocBlockSlicesJL (*alloc_t)(void *, AllocRequestJL *, size_t);
  typedef void (*free_t)(void *, AllocBlockSlicesJL);
  typedef void (*error_cb_t)(void *, const char *);
  typedef void (*addr_val)(void *, const char *, uintptr_t, uint64_t);

  struct RustMemoryInterface
  {
    void *state;

    ptr_t getfnPtr;
    alloc_t allocateJIT;
    free_t freeJITStructure;
    error_cb_t onError;
    addr_val storeAddr;
  };
}

class AllocationUnFinalized : public JITLinkMemoryManager::InFlightAlloc
{
  RustMemoryInterface *memory;

  uintptr_t rw_ptr;
  uintptr_t exec_ptr;

public:
  AllocationUnFinalized(RustMemoryInterface *memory, uintptr_t exec_ptr, uintptr_t rw_ptr) : memory(memory), exec_ptr(exec_ptr), rw_ptr(rw_ptr) {

                                                                                             };

  void finalize(OnFinalizedFunction OnFinalized) override
  {
    OnFinalized(JITLinkMemoryManager::FinalizedAlloc(orc::ExecutorAddr(exec_ptr)));
  }

  void abandon(OnAbandonedFunction OnAbandoned) override
  {
    OnAbandoned(Error::success());
  }
};

class JITAllocator : public JITLinkMemoryManager
{
  RustMemoryInterface *memory;

public:
  JITAllocator(RustMemoryInterface *memory) : memory(memory) {};

  void allocate(const JITLinkDylib *dylib, LinkGraph &G, OnAllocatedFunction onAlloc) override
  {
    std::vector<AllocRequestJL> Sizes = {};

    auto blocks = G.blocks();
    for (auto Block : blocks)
    {
      AllocRequestJL alloc;
      alloc.alignment = std::max((uint64_t)1, Block->getAlignment());
      alloc.size = Block->getSize();

      Sizes.push_back(alloc);
    }
    AllocRequestJL *pointerElm = static_cast<AllocRequestJL *>(Sizes.data());
    size_t len = Sizes.size();

    // Let rust figure it all out
    auto pointer = (memory->allocateJIT)(memory->state, pointerElm, len);

    if (pointer.len == 0 || !pointer.allocs)
    {
      onAlloc(Error::Error(make_error<StringError>("Could not allocate", inconvertibleErrorCode())));
      return;
    }

    std::span<AllocBlockSliceJL> allocsSpan(pointer.allocs, pointer.len);
    // Push the new addresses
    size_t idx = 0;
    for (auto block : blocks)
    {
      auto dest = reinterpret_cast<char *>(allocsSpan[idx].rwview);
      auto align = block->getAlignment();
      if (!block->isZeroFill())
      {
        auto originalContent = block->getContent();
        memcpy(dest, originalContent.data(), originalContent.size());
      }
      else
      {
        memset(dest, 0, block->getSize());
      }

      assert(allocsSpan[idx].rwview % align == 0);
      assert(allocsSpan[idx].rxview % align == 0);

      block->setAddress(
          orc::ExecutorAddr(allocsSpan[idx].rxview));
      block->setMutableContent(MutableArrayRef<char>(dest, block->getSize()));
      idx += 1;
    }

    auto InFlight = std::make_unique<AllocationUnFinalized>(memory, pointer.allocs->rxview, pointer.allocs->rwview);

    memory->freeJITStructure(memory->state, pointer);

    onAlloc(Expected<std::unique_ptr<JITLinkMemoryManager::InFlightAlloc>>(std::move(InFlight)));
  };

  // We do not deallocate the data as this structure is just used to populate our RWVIEW
  void deallocate(std::vector<FinalizedAlloc> allocs, OnDeallocatedFunction onDealloc) override
  {
    onDealloc(Error::success());
  };
};

class LinkContextProvider : public JITLinkContext
{
  RustMemoryInterface *memory;
  const JITLinkDylib *JD;

  JITAllocator *MemMgr;

public:
  LinkContextProvider(const JITLinkDylib *JD, RustMemoryInterface *memory) : JITLinkContext(JD), memory(memory), JD(JD)
  {
    MemMgr = new JITAllocator(this->memory);
  };

  JITAllocator &getMemoryManager() override
  {
    return *MemMgr;
  }

  ~LinkContextProvider()
  {
    delete MemMgr;
    delete JD;
  }

  void lookup(
      const LookupMap &symbols,
      std::unique_ptr<JITLinkAsyncLookupContinuation> LC) override
  {
    AsyncLookupResult Result;

    for (auto &kv : symbols)
    {
      auto name = kv.first;

      auto name_ref = *name;

      const char *ptr = name_ref.data();
      size_t len = name_ref.size();

      uintptr_t reloc = (memory->getfnPtr)(memory->state, ptr, len);

      Result[name] = orc::ExecutorSymbolDef(
          orc::ExecutorAddr(reloc),
          JITSymbolFlags::Exported);
    }

    LC->run(std::move(Result));
  }

  void notifyFailed(Error err) override
  {
    if (err)
    {
      handleAllErrors(std::move(err), [this](const ErrorInfoBase &E)
                      { 
        const auto message = E.message();
        const auto c_str = message.c_str();
        memory->onError(memory->state, c_str); });
    }
  };

  void notifyFinalized(JITLinkMemoryManager::FinalizedAlloc Alloc) override
  {
    Alloc.release();
  };

  Error notifyResolved(LinkGraph &graph) override
  {
    for (auto symbol : graph.defined_symbols())
    {
      if (symbol->hasName())
      {
        uint64_t addr = symbol->getAddress().getValue();
        auto symbolname = symbol->getName();
        auto csymbol = *symbolname;

        auto symbolnamedata = csymbol.data();
        auto symbollen = csymbol.size();

        memory->storeAddr(memory->state, symbolnamedata, symbollen, addr);
      }
    }
    return Error::success();
  };
};

extern "C"
{
  void *create_linkctx(RustMemoryInterface *linker)
  {
    const JITLinkDylib *dylib = new JITLinkDylib("mydylib");

    LinkContextProvider *ctx = new LinkContextProvider(dylib, linker);

    return ctx;
  }

  void *create_symbolpool()
  {
    orc::SymbolStringPool *ctx = new orc::SymbolStringPool();

    return ctx;
  }

  void free_symbolpool(void *symbolpool)
  {
    delete static_cast<orc::SymbolStringPool *>(symbolpool);
  }

  void free_linkctx(void *ctx_ptr)
  {
    delete static_cast<LinkContextProvider *>(ctx_ptr);
  }

  int link_consume_linkctx(void *ctx_ptr, void *symbol_pool, const char *data, size_t size)
  {
    LinkContextProvider *ctx_ptr_ref = static_cast<LinkContextProvider *>(ctx_ptr);
    auto ctx = std::unique_ptr<LinkContextProvider>(ctx_ptr_ref);

    StringRef ObjBuffer(data, size);
    auto Buffer = MemoryBuffer::getMemBuffer(ObjBuffer, "jit_object", false);

    auto symbolpool = std::shared_ptr<llvm::orc::SymbolStringPool>(
        static_cast<orc::SymbolStringPool *>(symbol_pool),
        [](auto *) {});

    auto G = createLinkGraphFromObject(Buffer->getMemBufferRef(), symbolpool);

    if (!G)
    {
      // Consume the error to avoid LLVM crashing on unhandled errors
      handleAllErrors(G.takeError(), [&](const ErrorInfoBase &E)
                      { 
        ctx->notifyFailed(make_error<StringError>(E.message(), inconvertibleErrorCode()));
        llvm::errs() << E.message(); });

      return 1;
    }

    link(std::move(*G), std::move(ctx));

    return 0;
  }
}