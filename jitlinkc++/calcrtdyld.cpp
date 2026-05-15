#include "llvm/Object/ObjectFile.h"
#include "llvm/ExecutionEngine/RuntimeDyld.h"
#include "llvm/ExecutionEngine/SectionMemoryManager.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/Error.h"
#include "calcsize.h"

#include <unordered_map>

using namespace llvm;
using namespace llvm::object;

class MemoryMgrDyld : public RuntimeDyld::MemoryManager
{
  std::vector<void *> allocs{};

public:
  std::vector<SizeAlignCS> sizealigns{};

  uint8_t *alloc(uintptr_t Size, unsigned Alignment, unsigned SectionID, StringRef Sect)
  {
#ifdef _MSC_VER
    void *allocation = _aligned_malloc(Size, Alignment);
#else
    void *allocation = aligned_alloc(Alignment, Size);
#endif

    allocs.push_back(allocation);

    SizeAlignCS algnt;

    algnt.size = Size;
    algnt.align = Alignment;
    sizealigns.push_back(
        algnt);

    return reinterpret_cast<uint8_t *>(allocation);
  }

public:
  MemoryMgrDyld() {}

  ~MemoryMgrDyld()
  {
    for (auto alloc : allocs)
    {
#ifdef _MSC_VER
      _aligned_free(alloc);
#else
      free(alloc);
#endif
    }
  }

  uint8_t *allocateCodeSection(uintptr_t Size, unsigned Alignment, unsigned SectionID, StringRef Sect)
  {
    return this->alloc(Size, Alignment, SectionID, Sect);
  }

  uint8_t *allocateDataSection(uintptr_t Size, unsigned Alignment, unsigned SectionID, StringRef Sect, bool IsReadOnly)
  {
    return this->alloc(Size, Alignment, SectionID, Sect);
  }

  // NOOP - not needed
  TLSSection allocateTLSSection(uintptr_t Size, unsigned Alignment, unsigned SectionID, StringRef SectionName)
  {
    return TLSSection(00);
  }
  void registerEHFrames(uint8_t *Addr, uint64_t LoadAddr, size_t Size)
  {
  }
  void deregisterEHFrames() {}

  bool finalizeMemory(std::string *ErrMsg = nullptr)
  {
    return true;
  }
};

class SymbolProvider : public JITSymbolResolver
{
public:
  SymbolProvider() {}

  void lookup(const LookupSet &Symbols, OnResolvedFunction OnResolved)
  {
    LookupResult results;
    for (auto symbol : Symbols)
    {
      auto data = symbol.data();
      auto len = symbol.size();

      results[symbol] = JITEvaluatedSymbol(
          JITTargetAddress(0),
          JITSymbolFlags::Exported);
    }

    OnResolved(Expected<LookupResult>(results));
  }

  Expected<LookupSet> getResponsibilitySet(const LookupSet &Symbols) override
  {
    return LookupSet(); // Dyld handles the responsibilities
  }
};

extern "C"
{
  int rtdyld_dryrun(void *state, sizealign_t sizealign, const char *data, size_t size)
  {
    auto MemMgr = std::make_unique<MemoryMgrDyld>();
    auto Resolver = std::make_unique<SymbolProvider>();
    RuntimeDyld RTDyld(*MemMgr, *Resolver);

    // Create LLVM memory buffer
    auto Buffer =
        MemoryBuffer::getMemBufferCopy(
            StringRef(
                reinterpret_cast<const char *>(data),
                size));

    auto object =
        ObjectFile::createObjectFile(
            Buffer->getMemBufferRef());

    if (!object)
    {
      return 1;
    }

    auto objout = std::move(object.get());

    RTDyld.loadObject(*objout);

    if (RTDyld.hasError())
    {
      llvm::errs() << RTDyld.getErrorString();
      return 1;
    }

    SizeAlignInfoCS sainfo;
    sainfo.ptr = MemMgr->sizealigns.data();
    sainfo.size = MemMgr->sizealigns.size();

    sizealign(state, sainfo);

    return 0;
  }
}
