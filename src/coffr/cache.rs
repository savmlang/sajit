use crate::coffr::arch::CompressedSectionView;

pub struct ReusableLinkerCache {
  idmap: Vec<u32>,
  views: Vec<CompressedSectionView>,
}

impl ReusableLinkerCache {
  pub fn create() -> Self {
    Self {
      idmap: vec![],
      views: vec![],
    }
  }
}

pub struct LinkerTransaction<'a>(&'a mut ReusableLinkerCache);

impl<'a> LinkerTransaction<'a> {
  pub fn create(cache: &'a mut ReusableLinkerCache) -> Self {
    cache.idmap.clear();
    cache.views.clear();

    Self(cache)
  }

  pub fn reserve(&mut self, additional: usize) {
    let len = self.0.idmap.len();
    let cap = self.0.idmap.capacity();
    let newcap = len + additional;

    const SHRINK_THRESHOLD: usize = 256;

    if cap > newcap && (cap - newcap) >= SHRINK_THRESHOLD {
      let target_cap = if newcap < 128 {
        128
      } else {
        (newcap * 115) / 100
      };

      self.0.idmap.shrink_to(target_cap);
      self.0.views.shrink_to(target_cap);
      return;
    }

    if cap >= newcap {
      return;
    }

    if newcap < 128 {
      self.0.idmap.reserve(additional);
      self.0.views.reserve(additional);
    } else {
      let target_cap = (newcap * 115) / 100;
      let delta = target_cap - len;

      self.0.idmap.reserve_exact(delta);
      self.0.views.reserve_exact(delta);
    }
  }

  #[inline]
  pub fn insert_sorted(&mut self, key: u32, item: CompressedSectionView) {
    debug_assert!(
      self.0.idmap.last().map_or(true, |&last| last < key),
      "Keys must be inserted in strictly ascending order"
    );

    self.0.views.push(item);
    self.0.idmap.push(key);
  }

  #[inline]
  pub fn get(&self, key: u32) -> Option<&CompressedSectionView> {
    let index: usize = self.0.idmap.binary_search(&key).ok()?;

    // Indices correspond 1:1
    Some(unsafe { self.0.views.get_unchecked(index) })
  }
}

impl<'a> Drop for LinkerTransaction<'a> {
  fn drop(&mut self) {
    self.0.idmap.clear();
    self.0.views.clear();
  }
}
