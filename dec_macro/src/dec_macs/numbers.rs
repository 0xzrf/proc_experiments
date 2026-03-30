fn print_result(num: i32) {
    println!("The result is {num}");
}

macro_rules! num {
    (one) => {
        1
    };
    (two) => {
        2
    };
    (three) => {
        3
    };
}

fn main() {
    print_result(num!(one) + num!(two) + num!(three));
}
