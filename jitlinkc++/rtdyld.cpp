#include "llvm/Object/ObjectFile.h"
#include "llvm/ExecutionEngine/RuntimeDyld.h"
#include "llvm/ExecutionEngine/SectionMemoryManager.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/Error.h"

#include <unordered_map>

using namespace llvm;
using namespace llvm::object;

extern "C"
{
  struct AllocBlockSliceRTDYLD
  {
    uintptr_t rwview;
    uintptr_t rxview;
  };

  struct AllocRequestRTDYLD
  {
    uintptr_t size;
    unsigned alignment;
  };

  struct SectionNameRTDYLD
  {
    const char *ptr;
    size_t size;
  };

  typedef void *(*getfn_ptr)(void *, const char *, size_t);
  typedef void (*offset_ptr)(void *, const char *, size_t, unsigned long long);

  typedef AllocBlockSliceRTDYLD (*allocate_t)(void *, AllocRequestRTDYLD, SectionNameRTDYLD);

  struct RustRTInterface
  {
    void *state;

    getfn_ptr getfnPtr;
    offset_ptr resolvefnOffset;
    allocate_t allocate;
  };
}

class MemoryMgrDyld : public RuntimeDyld::MemoryManager
{
  const RustRTInterface *rt;
  std::unordered_map<unsigned, AllocBlockSliceRTDYLD> *sectionsmap;

  uint8_t *alloc(uintptr_t Size, unsigned Alignment, unsigned SectionID, StringRef Sect)
  {
    AllocRequestRTDYLD alloc;
    alloc.alignment = Alignment;
    alloc.size = Size;

    SectionNameRTDYLD section;
    section.ptr = Sect.data();
    section.size = Sect.size();

    auto output = rt->allocate(rt->state, alloc, section);

    (*sectionsmap)[SectionID] = output;

    return reinterpret_cast<uint8_t *>(output.rwview);
  }

public:
  MemoryMgrDyld(const RustRTInterface *rt, std::unordered_map<unsigned, AllocBlockSliceRTDYLD> *sectionsmap) : rt(rt), sectionsmap(sectionsmap) {}

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
    return TLSSection();
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
  const RustRTInterface *rt;

public:
  SymbolProvider(const RustRTInterface *rt) : rt(rt) {}

  void lookup(const LookupSet &Symbols, OnResolvedFunction OnResolved)
  {
    LookupResult results;
    for (auto symbol : Symbols)
    {
      auto data = symbol.data();
      auto len = symbol.size();

      void *fnptr = rt->getfnPtr(rt->state, data, len);

      results[symbol] = JITEvaluatedSymbol(
          JITTargetAddress((unsigned long long)fnptr),
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
  int link_rtdyld(RustRTInterface *rt, const char *data, size_t size)
  {
    std::unordered_map<unsigned, AllocBlockSliceRTDYLD> sectionsmap;

    auto MemMgr = std::make_unique<MemoryMgrDyld>(rt, &sectionsmap);
    auto Resolver = std::make_unique<SymbolProvider>(rt);
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

    for (auto &[SectionID, alloc] : sectionsmap)
    {
      uint64_t rw =
          RTDyld.getSectionLoadAddress(SectionID);

      RTDyld.mapSectionAddress(
          (const void *)rw,
          alloc.rxview);
    }

    for (auto symbol : objout->symbols())
    {
      auto type = symbol.getType();

      if (!type)
      {
        consumeError(type.takeError());
        continue;
      }

      if (*type != SymbolRef::Type::ST_Function)
      {
        continue;
      }

      auto name = symbol.getName();

      if (!name)
      {
        consumeError(name.takeError());
        continue;
      }

      auto symbol_addr = symbol.getAddress();

      if (!symbol_addr)
      {
        consumeError(symbol_addr.takeError());
        continue;
      }

      auto nameptr = name->data();
      auto namesize = name->size();

      auto offset = *symbol_addr;

      rt->resolvefnOffset(rt->state, nameptr, namesize, offset);
    }

    RTDyld.resolveRelocations();

    if (RTDyld.hasError())
    {
      llvm::errs() << RTDyld.getErrorString();
      return 1;
    }

    return 0;
  }
}
