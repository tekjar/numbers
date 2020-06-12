#![feature(test)]
extern crate test;

use std::collections::HashMap;
use indexmap::IndexMap;
use fasthash::RandomState;
use fasthash::sea::Hash64;

fn iterate_vec(v: &Vec<Option<u64>>) -> u64 {
    let mut out = 0;
    for i in v {
        out += i.unwrap();
    }

    out
}

fn iterate_map(v: &HashMap<String, u64>) -> u64 {
    let mut out = 0;
    for (_, i) in v {
        out += *i;
    }

    out
}

fn iterate_indexmap(v: &IndexMap<String, u64>) -> u64 {
    let mut out = 0;
    for (_, i) in v {
        out += *i;
    }

    out
}

fn access_vec(v: &Vec<Option<u64>>, indices: &Vec<usize>) -> u64 {
    let mut out = 0;
    for i in indices {
        out += v.get(*i).unwrap().unwrap();
    }

    out
}

fn access_map(v: &HashMap<String, u64>, i: &str) -> u64 {
    *v.get(i).unwrap()
}

fn access_seahash_map(v: &HashMap<String, u64, RandomState<Hash64>>, i: &str) -> u64 {
    *v.get(i).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;
    use std::collections::HashMap;
    use rand::Rng;

    #[bench]
    fn iterate_vec_1_000_000(b: &mut Bencher) {
        let v = vec![Some(1); 1_000_000];
        b.iter(|| {
            iterate_vec(&v)
        });

        b.bytes = v.len() as u64 * 8;
    }

    #[bench]
    fn iterate_map_1_000_000(b: &mut Bencher) {
        let mut map = HashMap::new();
        for i in 0..1_000_000 {
            map.insert(i.to_string(), 1u64);
        }

        b.iter(|| {
            iterate_map(&map)
        });

        b.bytes = map.len() as u64 * 8;
    }

    #[bench]
    fn iterate_indexmap_1_000_000(b: &mut Bencher) {
        let mut map = IndexMap::new();
        for i in 0..1_000_000 {
            map.insert(i.to_string(), 1u64);
        }

        b.iter(|| {
            iterate_indexmap(&map)
        });

        b.bytes = map.len() as u64 * 8;
    }

    #[bench]
    fn access_vec_10(b: &mut Bencher) {
        let indices: Vec<usize> = (0..10).map(|_| rand::thread_rng().gen_range(0, 1_000_000)).collect();
        let v = vec![Some(1); 1_000_000];
        b.iter(|| {
            access_vec(&v, &indices)
        });
    }

    #[bench]
    fn access_map_1(b: &mut Bencher) {
        let mut map = HashMap::new();
        for i in 0..1_000_000 {
            let topic = "hello/world".to_owned() + &i.to_string();
            map.insert(topic, 1u64);
        }

        let i = "hello/world".to_owned() + &rand::thread_rng().gen_range(0, 1_000_000).to_string();
        b.iter(|| {
            access_map(&map, &i)
        });
    }

    #[bench]
    fn access_seahash_map_1(b: &mut Bencher) {
        let s = RandomState::<Hash64>::new();
        let mut map = HashMap::with_hasher(s);
        for i in 0..1_000_000 {
            let topic = "hello/world".to_owned() + &i.to_string();
            map.insert(topic, 1u64);
        }

        let i = "hello/world".to_owned() + &rand::thread_rng().gen_range(0, 1_000_000).to_string();
        b.iter(|| {
            access_seahash_map(&map, &i)
        });
    }
}



