use broken_app::{algo, leak_buffer, normalize, sum_even, use_after_free, concurrency};

fn main() {
    let nums = [1, 2, 3, 4];
    println!("sum_even: {}", sum_even(&nums));

    let data = [1_u8, 0, 2, 3];
    println!("non-zero bytes: {}", leak_buffer(&data));

    let text = " Hello World ";
    println!("normalize: {}", normalize(text));

    let fib = algo::slow_fib(20);
    println!("fib(20): {}", fib);

    let uniq = algo::slow_dedup(&[1, 2, 2, 3, 1, 4, 4]);
    println!("dedup: {:?}", uniq);

    let race = concurrency::race_increment(2, 2);
    println!("race: {}", race);

    println!("read_after_sleep: {}", concurrency::read_after_sleep());

    concurrency::reset_counter();

    // let uniq = algo::fast_dedup(&[1, 2, 2, 3, 1, 4, 4]);
    // println!("dedup: {:?}", uniq);

    unsafe{
        use_after_free();
    }

}
