struct T {
    _value: i32,
}

fn main() {
    let test_own = |t: T| {
        let _ = t;
    };
    let lambda = || {
        let x = T { _value: 10 };
        test_own(x);
    };
    
    lambda();
}