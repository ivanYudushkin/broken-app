use std::collections::HashSet;

pub fn slow_dedup(values: &[u64]) -> Vec<u64> {
    let mut vals = HashSet::new();

    for v in values {
        vals.insert(*v);
    }

    let mut out: Vec<u64> = vals.into_iter().collect();
    out.sort();  
    out
}

// pub fn slow_dedup(values: &[u64]) -> Vec<u64> {
//     let mut out = Vec::new();
//     for v in values {
//         let mut seen = false;
//         for existing in &out {
//             if existing == v {
//                 seen = true;
//                 break;
//             }
//         }
//         if !seen {
//             // лишняя копия, хотя можно было пушить значение напрямую
//             out.push(*v);// бесполезная сортировка на каждой вставке
//         }
//     }
//     out
// }

/// Классическая экспоненциальная реализация без мемоизации — будет медленной на больших n.
// pub fn slow_fib(n: u64) -> u64 {
//     match n {
//         0 => 0,
//         1 => 1,
//         _ => slow_fib(n - 1) + slow_fib(n - 2),
//     }
// }


pub fn slow_fib(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut a = 0u64;
    let mut b = 1u64;
    for _ in 2..=n {
        let next = a + b;
        a = b;
        b = next;
    }
    b
}
