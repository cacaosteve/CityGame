pub(super) fn format_number(value: u32) -> String {
    let text = value.to_string();
    let mut out = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub(super) fn format_signed_number(value: i32) -> String {
    let magnitude = format_number(value.unsigned_abs());
    if value < 0 {
        format!("-{magnitude}")
    } else {
        magnitude
    }
}
