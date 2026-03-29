////////// DO NOT CHANGE BELOW HERE /////////
fn print_result(num: i32) {
    println!("The result is {num}");
}
////////// DO NOT CHANGE ABOVE HERE /////////

macro_rules! math {
    ($operand1:literal plus $operand2:literal) => {
        $operand1 + $operand2
    };
    (square $val:literal) => {
        $val * $val
    };
}
////////// DO NOT CHANGE BELOW HERE /////////

fn main() {
    print_result(math!(3 plus 5));
    print_result(math!(square 2));
}
