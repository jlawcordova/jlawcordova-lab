//! An insertion-ordered map and set with JavaScript's `Map` and `Set`
//! semantics: setting a key that's there keeps its place, deleting one
//! closes the gap, and iteration is in insertion order. The Node engine's
//! outputs often depend on that order (pixels painted, colors first used,
//! copies made), so the port uses these wherever it used a `Map` or `Set`.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// A `Map`.
#[derive(Clone, Debug)]
pub struct OMap<K, V> {
    entries: Vec<(K, V)>,
    index: HashMap<K, usize>,
}

impl<K, V> Default for OMap<K, V> {
    fn default() -> Self {
        OMap { entries: Vec::new(), index: HashMap::new() }
    }
}

impl<K: Hash + Eq + Clone, V> OMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `map.set(k, v)`: a new key goes last, a known one keeps its place.
    pub fn set(&mut self, key: K, value: V) {
        if let Some(&i) = self.index.get(&key) {
            self.entries[i].1 = value;
        } else {
            self.index.insert(key.clone(), self.entries.len());
            self.entries.push((key, value));
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.index.get(key) {
            Some(&i) => Some(&mut self.entries[i].1),
            None => None,
        }
    }

    pub fn has<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.contains_key(key)
    }

    /// The value at `key`, set to `make()` first if it isn't there.
    pub fn get_or_insert_with(&mut self, key: K, make: impl FnOnce() -> V) -> &mut V {
        let i = match self.index.get(&key) {
            Some(&i) => i,
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, make()));
                self.entries.len() - 1
            }
        };
        &mut self.entries[i].1
    }

    /// `map.delete(k)`: returns the value it held.
    pub fn delete<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let i = self.index.remove(key)?;
        let (_, value) = self.entries.remove(i);
        for j in self.index.values_mut() {
            if *j > i {
                *j -= 1;
            }
        }
        Some(value)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.entries.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator {
        self.entries.iter_mut().map(|(_, v)| v)
    }

    pub fn into_entries(self) -> Vec<(K, V)> {
        self.entries
    }
}

/// Equal when the entries are equal in order.
impl<K: PartialEq, V: PartialEq> PartialEq for OMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<K: Hash + Eq + Clone, V> FromIterator<(K, V)> for OMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = OMap::new();
        for (k, v) in iter {
            map.set(k, v);
        }
        map
    }
}

/// A `Set`.
#[derive(Clone, Debug)]
pub struct OSet<K> {
    map: OMap<K, ()>,
}

impl<K> Default for OSet<K> {
    fn default() -> Self {
        OSet { map: OMap::default() }
    }
}

impl<K: Hash + Eq + Clone> OSet<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `set.add(k)`: returns whether it was new.
    pub fn add(&mut self, key: K) -> bool {
        if self.map.has(&key) {
            return false;
        }
        self.map.set(key, ());
        true
    }

    pub fn has<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.has(key)
    }

    pub fn delete<Q>(&mut self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.delete(key).is_some()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.map.keys()
    }
}

impl<K: PartialEq> PartialEq for OSet<K> {
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl<K: Hash + Eq + Clone> FromIterator<K> for OSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        let mut set = OSet::new();
        for k in iter {
            set.add(k);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_insertion_order_like_a_js_map() {
        let mut m = OMap::new();
        m.set("b", 1);
        m.set("a", 2);
        m.set("b", 3);
        assert_eq!(m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("b", 3), ("a", 2)]);
        m.delete("b");
        m.set("b", 4);
        assert_eq!(m.keys().copied().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(m.get("a"), Some(&2));
    }
}
