// Simple example Rust function
fn calculate_fibonacci(n: u32) -> u64 {
    if n <= 1 {
        return n as u64;
    }
    
    let mut a = 0u64;
    let mut b = 1u64;
    
    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    
    b
}

fn main() {
    let result = calculate_fibonacci(10);
    println!("Fibonacci of 10 is: {}", result);
}
