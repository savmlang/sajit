#include "llvm/ExecutionEngine/JITLink/JITLink.h"
#include "llvm/Support/MemoryBuffer.h"
#include <span>

using namespace llvm;
using namespace llvm::jitlink;

extern "C"
{
  struct SizeAlignCS
  {
    size_t size;
    uint64_t align;
  };

  struct SizeAlignInfoCS
  {
    SizeAlignCS *ptr;
    size_t size;
  };

  typedef void (*sizealign_t)(void *, SizeAlignInfoCS info);
};

class JITAllocator : public JITLinkMemoryManager
{
  sizealign_t sizealign_call;
  void *state;

public:
  JITAllocator(sizealign_t sizealign_call, void *state) : sizealign_call(sizealign_call), state(state) {};

  void allocate(const JITLinkDylib *dylib, LinkGraph &G, OnAllocatedFunction onAlloc) override
  {
    std::vector<SizeAlignCS> sizealign{};

    auto blocks = G.blocks();
    for (auto Block : blocks)
    {
      auto align = Block->getAlignment();
      auto blocksize = Block->getSize();

      SizeAlignCS data;
      data.align = align;
      data.size = blocksize;

      sizealign.push_back(data);
    }

    SizeAlignInfoCS sizealigninfo;
    sizealigninfo.ptr = sizealign.data();
    sizealigninfo.size = sizealign.size();

    (sizealign_call)(state, sizealigninfo);

    onAlloc(AllocResult(llvm::make_error<StringError>("Could not generate", inconvertibleErrorCode())));
  };

  void deallocate(std::vector<FinalizedAlloc> allocs, OnDeallocatedFunction onDealloc) override
  {
    onDealloc(Error::success());
  };
};

class LinkContextProvider : public JITLinkContext
{
  const JITLinkDylib *JD;

  JITAllocator *MemMgr;

public:
  LinkContextProvider(const JITLinkDylib *JD, sizealign_t sizealign, void *state) : JITLinkContext(JD), JD(JD)
  {
    MemMgr = new JITAllocator(sizealign, state);
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

      Result[name] = orc::ExecutorSymbolDef(
          orc::ExecutorAddr(0),
          JITSymbolFlags::Exported);
    }

    LC->run(std::move(Result));
  }

  void notifyFailed(Error err) override
  {
    consumeError(std::move(err));
  };

  void notifyFinalized(JITLinkMemoryManager::FinalizedAlloc Alloc) override
  {
    Alloc.release();
  };

  Error notifyResolved(LinkGraph &graph) override
  {
    return Error::success();
  };
};

extern "C"
{
  int dryrun_link(void *state, sizealign_t sizealigncall, const char *data, size_t size)
  {
    const JITLinkDylib *dylib = new JITLinkDylib("jitlink");
    auto ctx = std::make_unique<LinkContextProvider>(dylib, sizealigncall, state);

    StringRef ObjBuffer(data, size);
    auto Buffer = MemoryBuffer::getMemBuffer(ObjBuffer, "jit_object", false);

    auto symbolpool = std::make_shared<llvm::orc::SymbolStringPool>();

    auto G = createLinkGraphFromObject(Buffer->getMemBufferRef(), symbolpool);
    if (!G)
    {
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