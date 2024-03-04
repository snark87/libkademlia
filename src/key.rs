use generic_array::{ArrayLength, GenericArray};
use rand::{rngs::OsRng, Rng};

use crate::params::KeySizeParameters;

/// Represents a Key in the Kademlia Distributed Hash Table (DHT).
///
/// In Kademlia, a Key is a unique identifier used for both nodes and data entries within the DHT network.
///
/// The Key struct is designed to work seamlessly with the XOR metric for distance calculation, which is
/// central to Kademlia's routing algorithm.
///
/// # Type parameters
///
/// - `Params`: defines size of key
///
/// # Example
///
/// Generate random key
/// ```
/// use kademlia::Key;
/// use kademlia::DefaultKademliaParameters;
/// // ...
/// let key = Key::<DefaultKademliaParameters>::new();
/// ````
#[derive(Clone)]
pub struct Key<Params: KeySizeParameters>(pub GenericArray<u8, Params::KeySize>);

/// Measures distance between two keys
#[derive(Clone)]
pub struct Distance<Params: KeySizeParameters>(pub GenericArray<u8, Params::KeySize>);

pub trait KeyLike<P: KeySizeParameters>: AsRef<[u8]> {
    fn iter(&self) -> impl Iterator<Item=&u8> {
        let slice = self.as_ref();
        slice.iter()
    }
}

impl<P: KeySizeParameters> KeyLike<P> for Key<P> {}

impl<P: KeySizeParameters> Key<P> {
    pub fn zero() -> Self {
        Key(GenericArray::default())
    }

    pub fn new() -> Self {
        let mut key: Self = Self::zero();
        let mut rng = OsRng;
        rng.fill(key.0.as_mut_slice());

        key
    }
}

impl<P: KeySizeParameters> AsRef<[u8]> for Key<P> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<P: KeySizeParameters> PartialEq for Key<P> {
    fn eq(&self, other: &Self) -> bool {
        Distance::<P>::between(self, other).is_zero()
    }
}

impl<P: KeySizeParameters> Eq for Key<P>{}

impl<P: KeySizeParameters> std::hash::Hash for Key<P>{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<Params: KeySizeParameters> Distance<Params> {
    pub fn between<K: KeyLike<Params>>(key1: &K, key2: &K) -> Distance<Params> {
        let mut data: GenericArray<u8, Params::KeySize> = GenericArray::default();

        let zipped_keys = key1.iter().zip(key2.iter());
        zipped_keys
            .map(|(x, y)| x ^ y)
            .enumerate()
            .for_each(|(i, x)| data[i] = x);

        Distance(data)
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
}

fn write_byte_string<N: ArrayLength>(
    array: &GenericArray<u8, N>,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    write!(f, "0x")?;
    array.iter().map(|el| write!(f, "{:x}", el)).collect()
}

impl<P: KeySizeParameters> std::fmt::Debug for Key<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Key {{")?;
        write_byte_string(&self.0, f)?;
        write!(f, "}}")
    }
}

impl<P: KeySizeParameters> std::fmt::Debug for Distance<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "distance: ")?;
        write_byte_string(&self.0, f)
    }
}
impl <P: KeySizeParameters> PartialEq for Distance<P> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl <P: KeySizeParameters> PartialOrd for Distance<P> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl <P: KeySizeParameters> Eq for Distance<P> {
}

impl <P: KeySizeParameters> Ord for Distance<P> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::params::DefaultKademliaParameters;

    use super::{Distance, Key};

    type TestKey = Key<DefaultKademliaParameters>;

    #[test]
    pub fn zero_keys_should_be_equal() {
        let key1 = TestKey::zero();
        let key2 = TestKey::zero();
        assert_eq!(key1, key2)
    }

    #[test]
    fn zero_distance_should_be_less_than_anything() {
        let key1 = TestKey::new();
        let key2 = TestKey::new();
        let zero_distance = Distance::between(&key1, &key1);
        let non_zero_distance = Distance::between(&key1, &key2);
        assert!(zero_distance.is_zero());
        assert!(!non_zero_distance.is_zero());
        assert!(zero_distance < non_zero_distance);
    }
}
