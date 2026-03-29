////////// DO NOT CHANGE BELOW HERE /////////
#[derive(Debug)]
struct Coordinate {
    x: i32,
    y: i32,
}

impl Coordinate {
    fn show(&self) {
        println!("({}, {})", self.x, self.y);
    }
}

////////// DO NOT CHANGE ABOVE HERE /////////
macro_rules! for_2d {
    (
        $var1:ident <$var1_type:ty> in $iter1:expr,
        $var2:ident <$var2_type:ty> in $iter2:expr,
        $to_run:block
    ) => {
        for $var1 in $iter1 {
            let $var1: $var1_type = $var1;
            for $var2 in $iter2 {
                let $var2: $var2_type = $var2;
                $to_run
            }
        }
    };
}
////////// DO NOT CHANGE BELOW HERE /////////

fn main() {
    for_2d!(row <i32> in 1..5, col <i32> in 2..7, {
        (Coordinate {x: col, y: row}).show()
    });

    let values = [1, 3, 5];

    for_2d!(x <u16> in values, y <u16> in values, {
        (Coordinate {x: x.into(), y: y.into()}).show()
    });
}
