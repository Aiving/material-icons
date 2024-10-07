include!(concat!(env!("OUT_DIR"), "/icons.rs"));

#[cfg(test)]
mod tests {
    use crate::{icon_downloading, IconStyle};
    use core::str;

    #[test]
    fn test_icon() {
        println!(
            "{}",
            str::from_utf8(icon_downloading(IconStyle::Outlined, 0, 400, 24)).unwrap()
        )
    }
}
