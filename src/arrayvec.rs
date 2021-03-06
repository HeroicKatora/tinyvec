use super::*;

/// Helper to make an `ArrayVec`.
///
/// You specify the backing array type, and optionally give all the elements you
/// want to initially place into the array.
///
/// As an unfortunate restriction, the backing array type must support `Default`
/// for it to work with this macro.
///
/// ```rust
/// use tinyvec::*;
/// 
/// let empty_av = array_vec!([u8; 16]);
/// 
/// let some_ints = array_vec!([i32; 4], 1, 2, 3);
/// ```
#[macro_export]
macro_rules! array_vec {
  ($array_type:ty) => {
    {
      let av: ArrayVec<$array_type> = Default::default();
      av
    }
  };
  ($array_type:ty, $($elem:expr),*) => {
    {
      let mut av: ArrayVec<$array_type> = Default::default();
      $( av.push($elem); )*
      av
    }
  };
}

/// An array-backed vector-like data structure.
///
/// * Fixed capacity (based on array size).
/// * Variable length.
/// * All of the array memory is always "initialized" in the init/uninit memory
///   sense.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ArrayVec<A: Array> {
  len: usize,
  data: A,
}

impl<A: Array> Deref for ArrayVec<A> {
  type Target = [A::Item];
  #[inline(always)]
  #[must_use]
  fn deref(&self) -> &Self::Target {
    &self.data.as_slice()[..self.len]
  }
}

impl<A: Array> DerefMut for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.data.as_slice_mut()[..self.len]
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> Index<I> for ArrayVec<A> {
  type Output = <I as SliceIndex<[A::Item]>>::Output;
  #[inline(always)]
  #[must_use]
  fn index(&self, index: I) -> &Self::Output {
    &self.deref()[index]
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> IndexMut<I> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    &mut self.deref_mut()[index]
  }
}

impl<A: Array> ArrayVec<A> {
  /// Move all values from `other` into this vec.
  /// 
  /// ## Panics
  /// * If the vec overflows its capacity
  /// 
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 10], 1, 2, 3);
  /// let mut av2 = array_vec!([i32; 10], 4, 5, 6);
  /// av.append(&mut av2);
  /// assert_eq!(av, &[1, 2, 3, 4, 5, 6][..]);
  /// assert_eq!(av2, &[][..]);
  /// ```
  #[inline]
  pub fn append(&mut self, other: &mut Self) {
    for item in other.drain(..) {
      self.push(item)
    }
  }

  /// A mutable pointer to the backing array.
  /// 
  /// ## Safety
  /// 
  /// This pointer has provenance over the _entire_ backing array.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_ptr(&mut self) -> *mut A::Item {
    self.data.as_slice_mut().as_mut_ptr()
  }

  /// Helper for getting the mut slice.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
    self.deref_mut()
  }

  /// A const pointer to the backing array.
  /// 
  /// ## Safety
  /// 
  /// This pointer has provenance over the _entire_ backing array.
  #[inline(always)]
  #[must_use]
  pub fn as_ptr(&self) -> *const A::Item {
    self.data.as_slice().as_ptr()
  }

  /// Helper for getting the shared slice.
  #[inline(always)]
  #[must_use]
  pub fn as_slice(&self) -> &[A::Item] {
    self.deref()
  }

  /// The capacity of the `ArrayVec`.
  /// 
  /// This is fixed based on the array type.
  #[inline(always)]
  #[must_use]
  pub fn capacity(&self) -> usize {
    A::CAPACITY
  }

  /// Removes all elements from the vec.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.truncate(0)
  }

  /// De-duplicates the vec.
  #[cfg(feature = "nightly_slice_partition_dedup")]
  #[inline(always)]
  pub fn dedup(&mut self)
  where
    A::Item: PartialEq,
  {
    self.dedup_by(|a, b| a == b)
  }

  /// De-duplicates the vec according to the predicate given.
  #[cfg(feature = "nightly_slice_partition_dedup")]
  #[inline(always)]
  pub fn dedup_by<F>(&mut self, same_bucket: F)
  where
    F: FnMut(&mut A::Item, &mut A::Item) -> bool,
  {
    let len = {
      let (dedup, _) = self.as_mut_slice().partition_dedup_by(same_bucket);
      dedup.len()
    };
    self.truncate(len);
  }

  /// De-duplicates the vec according to the key selector given.
  #[cfg(feature = "nightly_slice_partition_dedup")]
  #[inline(always)]
  pub fn dedup_by_key<F, K>(&mut self, mut key: F)
  where
    F: FnMut(&mut A::Item) -> K,
    K: PartialEq,
  {
    self.dedup_by(|a, b| key(a) == key(b))
  }

  /// Creates a draining iterator that removes the specified range in the vector
  /// and yields the removed items.
  ///
  /// ## Panics
  /// * If the start is greater than the end
  /// * If the end is past the edge of the vec.
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4], 1, 2, 3);
  /// let av2: ArrayVec<[i32; 4]> = av.drain(1..).collect();
  /// assert_eq!(av.as_slice(), &[1][..]);
  /// assert_eq!(av2.as_slice(), &[2, 3][..]);
  ///
  /// av.drain(..);
  /// assert_eq!(av.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn drain<R: RangeBounds<usize>>(
    &mut self,
    range: R,
  ) -> ArrayVecDrain<'_, A> {
    use core::ops::Bound;
    let start = match range.start_bound() {
      Bound::Included(x) => *x,
      Bound::Excluded(x) => x + 1,
      Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
      Bound::Included(x) => x + 1,
      Bound::Excluded(x) => *x,
      Bound::Unbounded => self.len,
    };
    assert!(
      start <= end,
      "ArrayVec::drain> Illegal range, {} to {}",
      start,
      end
    );
    assert!(
      end <= self.len,
      "ArrayVec::drain> Range ends at {} but length is only {}!",
      end,
      self.len
    );
    ArrayVecDrain {
      parent: self,
      target_index: start,
      target_count: end - start,
    }
  }

  // LATER(Vec): drain_filter #nightly https://github.com/rust-lang/rust/issues/43244

  /// Clone each element of the slice into this vec.
  #[inline]
  pub fn extend_from_slice(&mut self, sli: &[A::Item])
  where
    A::Item: Clone,
  {
    for i in sli {
      self.push(i.clone())
    }
  }

  /// Wraps up an array and uses the given length as the initial length.
  ///
  /// Note that the `From` impl for arrays assumes the full length is used.
  ///
  /// ## Panics
  ///
  /// The length must be less than or equal to the capacity of the array.
  #[inline]
  #[must_use]
  #[allow(clippy::match_wild_err_arm)]
  pub fn from_array_len(data: A, len: usize) -> Self {
    match Self::try_from_array_len(data, len) {
      Ok(out) => out,
      Err(_) => {
        panic!("ArrayVec: length {} exceeds capacity {}!", len, A::CAPACITY)
      }
    }
  }

  /// Inserts an item at the position given, moving all following elements +1
  /// index.
  ///
  /// ## Panics
  /// * If `index` > `len`
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 10], 1, 2, 3);
  /// av.insert(1, 4);
  /// assert_eq!(av.as_slice(), &[1, 4, 2, 3]);
  /// av.insert(4, 5);
  /// assert_eq!(av.as_slice(), &[1, 4, 2, 3, 5]);
  /// ```
  #[inline]
  pub fn insert(&mut self, index: usize, item: A::Item) {
    use core::cmp::Ordering;
    match index.cmp(&self.len) {
      Ordering::Less => {
        let targets: &mut [A::Item] = &mut self.as_mut_slice()[index..];
        let mut temp = item;
        for target in targets.iter_mut() {
          temp = replace(target, temp);
        }
        self.push(temp);
      }
      Ordering::Equal => {
        self.push(item);
      }
      Ordering::Greater => {
        panic!(
          "ArrayVec::insert> index {} is out of bounds {}",
          index, self.len
        );
      }
    }
  }

  /// If the vec is empty.
  #[inline(always)]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  /// The length of the vec (in elements).
  #[inline(always)]
  #[must_use]
  pub fn len(&self) -> usize {
    self.len
  }

  /// Makes a new, empty vec.
  #[inline(always)]
  #[must_use]
  pub fn new() -> Self
  where
    A: Default,
  {
    Self::default()
  }

  /// Remove and return the last element of the vec, if there is one.
  /// 
  /// ## Failure
  /// * If the vec is empty you get `None`.
  #[inline]
  pub fn pop(&mut self) -> Option<A::Item> {
    if self.len > 0 {
      self.len -= 1;
      let out =
        replace(&mut self.data.as_slice_mut()[self.len], A::Item::default());
      Some(out)
    } else {
      None
    }
  }

  /// Place an element onto the end of the vec.
  /// 
  /// ## Panics
  /// * If the length of the vec would overflow the capacity.
  #[inline(always)]
  pub fn push(&mut self, val: A::Item) {
    if self.len < A::CAPACITY {
      replace(&mut self.data.as_slice_mut()[self.len], val);
      self.len += 1;
    } else {
      panic!("ArrayVec: overflow!")
    }
  }

  /// Removes the item at `index`, shifting all others down by one index.
  ///
  /// Returns the removed element.
  ///
  /// ## Panics
  ///
  /// If the index is out of bounds.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4], 1, 2, 3);
  /// assert_eq!(av.remove(1), 2);
  /// assert_eq!(av.as_slice(), &[1, 3][..]);
  /// ```
  #[inline]
  pub fn remove(&mut self, index: usize) -> A::Item {
    let targets: &mut [A::Item] = &mut self.deref_mut()[index..];
    let mut spare = A::Item::default();
    for target in targets.iter_mut().rev() {
      spare = replace(target, spare);
    }
    self.len -= 1;
    spare
  }

  // NIGHTLY: remove_item, https://github.com/rust-lang/rust/issues/40062

  /// Resize the vec to the new length.
  ///
  /// If it needs to be longer, it's filled with clones of the provided value.
  /// If it needs to be shorter, it's truncated.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut av = array_vec!([&str; 10], "hello");
  /// av.resize(3, "world");
  /// assert_eq!(av.as_slice(), &["hello", "world", "world"][..]);
  ///
  /// let mut av = array_vec!([i32; 10], 1, 2, 3, 4);
  /// av.resize(2, 0);
  /// assert_eq!(av.as_slice(), &[1, 2][..]);
  /// ```
  #[inline]
  pub fn resize(&mut self, new_len: usize, new_val: A::Item)
  where
    A::Item: Clone,
  {
    use core::cmp::Ordering;
    match new_len.cmp(&self.len) {
      Ordering::Less => self.truncate(new_len),
      Ordering::Equal => (),
      Ordering::Greater => {
        while self.len < new_len {
          self.push(new_val.clone());
        }
      }
    }
  }

  /// Resize the vec to the new length.
  ///
  /// If it needs to be longer, it's filled with repeated calls to the provided
  /// function. If it needs to be shorter, it's truncated.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut av = array_vec!([i32; 10], 1, 2, 3);
  /// av.resize_with(5, Default::default);
  /// assert_eq!(av.as_slice(), &[1, 2, 3, 0, 0][..]);
  ///
  /// let mut av = array_vec!([i32; 10]);
  /// let mut p = 1;
  /// av.resize_with(4, || {
  ///   p *= 2;
  ///   p
  /// });
  /// assert_eq!(av.as_slice(), &[2, 4, 8, 16][..]);
  /// ```
  #[inline]
  pub fn resize_with<F: FnMut() -> A::Item>(
    &mut self,
    new_len: usize,
    mut f: F,
  ) {
    use core::cmp::Ordering;
    match new_len.cmp(&self.len) {
      Ordering::Less => self.truncate(new_len),
      Ordering::Equal => (),
      Ordering::Greater => {
        while self.len < new_len {
          self.push(f());
        }
      }
    }
  }

  /// Walk the vec and keep only the elements that pass the predicate given.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut av = array_vec!([i32; 10], 1, 1, 2, 3, 3, 4);
  /// av.retain(|&x| x % 2 == 0);
  /// assert_eq!(av.as_slice(), &[2, 4][..]);
  /// ```
  #[inline]
  pub fn retain<F: FnMut(&A::Item) -> bool>(&mut self, mut acceptable: F) {
    let mut i = 0;
    while i < self.len {
      if !acceptable(&self[i]) {
        self.remove(i);
      } else {
        i += 1;
      }
    }
  }

  /// Forces the length of the vector to `new_len`.
  ///
  /// ## Panics
  /// If `new_len` is greater than the vec's capacity.
  ///
  /// ## Safety
  /// * This is a fully safe operation! The inactive memory already counts as
  ///   "initialized" by Rust's rules.
  /// * Other than "the memory is initialized" there are no other guarantees
  ///   regarding what you find in the inactive portion of the vec.
  #[inline(always)]
  pub fn set_len(&mut self, new_len: usize) {
    if new_len > A::CAPACITY {
      // Note(Lokathor): Technically we don't have to panic here, and we could
      // just let some other call later on trigger a panic on accident when the
      // length is wrong. However, it's a lot easier to catch bugs when things
      // are more "fail-fast".
      panic!("ArrayVec: set_len overflow!")
    } else {
      self.len = new_len;
    }
  }

  /// Splits the collection at the point given.
  ///
  /// * `[0, at)` stays in this vec
  /// * `[at, len)` ends up in the new vec.
  ///
  /// ## Panics
  /// * if at > len
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4], 1, 2, 3);
  /// let av2 = av.split_off(1);
  /// assert_eq!(av.as_slice(), &[1][..]);
  /// assert_eq!(av2.as_slice(), &[2, 3][..]);
  /// ```
  #[inline]
  pub fn split_off(&mut self, at: usize) -> Self
  where
    Self: Default,
  {
    // FIXME: should this just use drain into the output?
    if at > self.len {
      panic!(
        "ArrayVec::split_off> at value {} exceeds length of {}",
        at, self.len
      );
    }
    let mut new = Self::default();
    let moves = &mut self.as_mut_slice()[at..];
    let targets = new.data.as_slice_mut();
    for (m, t) in moves.iter_mut().zip(targets) {
      replace(t, replace(m, A::Item::default()));
    }
    new.len = self.len - at;
    self.len = at;
    new
  }

  /// Remove an element, swapping the end of the vec into its place.
  ///
  /// ## Panics
  /// * If the index is out of bounds.
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([&str; 4], "foo", "bar", "quack", "zap");
  ///
  /// assert_eq!(av.swap_remove(1), "bar");
  /// assert_eq!(av.as_slice(), &["foo", "zap", "quack"][..]);
  ///
  /// assert_eq!(av.swap_remove(0), "foo");
  /// assert_eq!(av.as_slice(), &["quack", "zap"][..]);
  /// ```
  #[inline]
  pub fn swap_remove(&mut self, index: usize) -> A::Item {
    assert!(
      index < self.len,
      "ArrayVec::swap_remove> index {} is out of bounds {}",
      index,
      self.len
    );
    if index == self.len - 1 {
        self.pop().unwrap()
    } else {
        let i = self.pop().unwrap();
        replace(&mut self[index], i)
    }
  }

  /// Reduces the vec's length to the given value.
  /// 
  /// If the vec is already shorter than the input, nothing happens.
  #[inline]
  pub fn truncate(&mut self, new_len: usize) {
    if needs_drop::<A::Item>() {
      while self.len > new_len {
        self.pop();
      }
    } else {
      self.len = self.len.min(new_len);
    }
  }

  /// Wraps an array, using the given length as the starting length.
  ///
  /// If you want to use the whole length of the array, you can just use the
  /// `From` impl.
  ///
  /// ## Failure
  ///
  /// If the given length is greater than the capacity of the array this will
  /// error, and you'll get the array back in the `Err`.
  #[inline]
  pub fn try_from_array_len(data: A, len: usize) -> Result<Self, A> {
    if len <= A::CAPACITY {
      Ok(Self { data, len })
    } else {
      Err(data)
    }
  }

  /// Obtain the shared slice of the array _after_ the active memory.
  /// 
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4]);
  /// assert_eq!(av.grab_spare_slice().len(), 4);
  /// av.push(10);
  /// av.push(11);
  /// av.push(12);
  /// av.push(13);
  /// assert_eq!(av.grab_spare_slice().len(), 0);
  /// ```
  #[inline(always)]
  #[cfg(feature = "grab_spare_slice")]
  pub fn grab_spare_slice(&self) -> &[A::Item] {
    &self.data.as_slice()[self.len..]
  }
  
  /// Obtain the mutable slice of the array _after_ the active memory.
  /// 
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4]);
  /// assert_eq!(av.grab_spare_slice_mut().len(), 4);
  /// av.push(10);
  /// av.push(11);
  /// assert_eq!(av.grab_spare_slice_mut().len(), 2);
  /// ```
  #[inline(always)]
  #[cfg(feature = "grab_spare_slice")]
  pub fn grab_spare_slice_mut(&mut self) -> &mut [A::Item] {
    &mut self.data.as_slice_mut()[self.len..]
  }
}

/// Draining iterator for `ArrayVecDrain`
/// 
/// See [`ArrayVecDrain::drain`](ArrayVecDrain::<A>::drain)
pub struct ArrayVecDrain<'p, A: Array> {
  parent: &'p mut ArrayVec<A>,
  target_index: usize,
  target_count: usize,
}
// GoodFirstIssue: this entire type is correct but slow.
// NIGHTLY: vec_drain_as_slice, https://github.com/rust-lang/rust/issues/58957
impl<'p, A: Array> Iterator for ArrayVecDrain<'p, A> {
  type Item = A::Item;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.target_count > 0 {
      let out = self.parent.remove(self.target_index);
      self.target_count -= 1;
      Some(out)
    } else {
      None
    }
  }
}
impl<'p, A: Array> Drop for ArrayVecDrain<'p, A> {
  #[inline]
  fn drop(&mut self) {
    for _ in self {}
  }
}

impl<A: Array> AsMut<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn as_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> AsRef<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn as_ref(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> Borrow<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn borrow(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> BorrowMut<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  fn borrow_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> Extend<A::Item> for ArrayVec<A> {
  #[inline]
  fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
    for t in iter {
      self.push(t)
    }
  }
}

impl<A: Array> From<A> for ArrayVec<A> {
  #[inline(always)]
  #[must_use]
  /// The output has a length equal to the full array.
  ///
  /// If you want to select a length, use
  /// [`from_array_len`](ArrayVec::from_array_len)
  fn from(data: A) -> Self {
    Self { len: data.as_slice().len(), data }
  }
}

impl<A: Array + Default> FromIterator<A::Item> for ArrayVec<A> {
  #[inline]
  #[must_use]
  fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
    let mut av = Self::default();
    for i in iter {
      av.push(i)
    }
    av
  }
}

/// Iterator for consuming an `ArrayVec` and returning owned elements.
pub struct ArrayVecIterator<A: Array> {
  base: usize,
  len: usize,
  data: A,
}
impl<A: Array> Iterator for ArrayVecIterator<A> {
  type Item = A::Item;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.base < self.len {
      let out =
        replace(&mut self.data.as_slice_mut()[self.base], A::Item::default());
      self.base += 1;
      Some(out)
    } else {
      None
    }
  }
  #[inline(always)]
  #[must_use]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let s = self.len - self.base;
    (s, Some(s))
  }
  #[inline(always)]
  fn count(self) -> usize {
    self.len - self.base
  }
  #[inline]
  fn last(mut self) -> Option<Self::Item> {
    Some(replace(&mut self.data.as_slice_mut()[self.len], A::Item::default()))
  }
  #[inline]
  fn nth(&mut self, n: usize) -> Option<A::Item> {
    let i = self.base + (n - 1);
    if i < self.len {
      let out = replace(&mut self.data.as_slice_mut()[i], A::Item::default());
      self.base = i + 1;
      Some(out)
    } else {
      None
    }
  }
}

impl<A: Array> IntoIterator for ArrayVec<A> {
  type Item = A::Item;
  type IntoIter = ArrayVecIterator<A>;
  #[inline(always)]
  #[must_use]
  fn into_iter(self) -> Self::IntoIter {
    ArrayVecIterator { base: 0, len: self.len, data: self.data }
  }
}

impl<A: Array> PartialEq for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  #[must_use]
  fn eq(&self, other: &Self) -> bool {
    self.deref().eq(other.deref())
  }
}
impl<A: Array> Eq for ArrayVec<A> where A::Item: Eq {}

impl<A: Array> PartialOrd for ArrayVec<A>
where
  A::Item: PartialOrd,
{
  #[inline]
  #[must_use]
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    self.deref().partial_cmp(other.deref())
  }
}
impl<A: Array> Ord for ArrayVec<A>
where
  A::Item: Ord,
{
  #[inline]
  #[must_use]
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    self.deref().cmp(other.deref())
  }
}

impl<A: Array> PartialEq<&A> for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  #[must_use]
  fn eq(&self, other: &&A) -> bool {
    self.deref() == other.as_slice()
  }
}

impl<A: Array> PartialEq<&[A::Item]> for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  #[must_use]
  fn eq(&self, other: &&[A::Item]) -> bool {
    self.deref() == *other
  }
}

/*

I think, in retrospect, this is useless?

The `&mut [A::Item]` should coerce to `&[A::Item]` and use the above impl.
I'll leave it here for now though since we already had it written out..

impl<A: Array> PartialEq<&mut [A::Item]> for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  #[must_use]
  fn eq(&self, other: &&mut [A::Item]) -> bool {
    self.deref() == *other
  }
}
*/

// //
// Formatting impls
// //

impl<A: Array> Binary for ArrayVec<A>
where
  A::Item: Binary,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      Binary::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> Debug for ArrayVec<A>
where
  A::Item: Debug,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      Debug::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> Display for ArrayVec<A>
where
  A::Item: Display,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      Display::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> LowerExp for ArrayVec<A>
where
  A::Item: LowerExp,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      LowerExp::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> LowerHex for ArrayVec<A>
where
  A::Item: LowerHex,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      LowerHex::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> Octal for ArrayVec<A>
where
  A::Item: Octal,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      Octal::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> Pointer for ArrayVec<A>
where
  A::Item: Pointer,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      Pointer::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> UpperExp for ArrayVec<A>
where
  A::Item: UpperExp,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      UpperExp::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}

impl<A: Array> UpperHex for ArrayVec<A>
where
  A::Item: UpperHex,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      UpperHex::fmt(elem, f)?;
    }
    write!(f, "]")
  }
}
