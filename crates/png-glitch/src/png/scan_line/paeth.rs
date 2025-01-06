pub fn predict(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i8;
    let b = b as i8;
    let c = c as i8;

    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    let pr = if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    };
    pr as u8
}
