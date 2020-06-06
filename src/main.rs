use lfuda::CacheBuilder;
use lfuda::CachePolicy::LFUDA;

fn main() -> Result<(), &'static str> {
    let c: CacheBuilder<i32, i32> = CacheBuilder::new()
        .set_max_capacity(3)
        .set_max_size(120)
        .set_policy(LFUDA);

    let mut cache = c.build();
    cache.insert(3, 3, 0, None).unwrap();
    cache.insert(3, 3, 0, None).unwrap();
    cache.insert(3, 3, 0, None).unwrap();
    cache.insert(3, 3, 0, None).unwrap();

    for _ in 1..7 {
        println!("{:?}", cache.peek(&3))
    }
    Ok(())
}
