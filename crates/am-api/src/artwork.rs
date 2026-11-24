// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

/// Fills an artwork URL template. Replaces `{w}` and `{h}` with the size, `{c}` with the
/// crop code `bb` (the pages that ask for `art[url]=c,f` get this placeholder), and `{f}` with
/// `jpg`.
#[must_use]
pub fn artwork_url(template: &str, width: u32, height: u32) -> String {
    template
        .replace("{w}", &width.to_string())
        .replace("{h}", &height.to_string())
        .replace("{c}", "bb")
        .replace("{f}", "jpg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_url_replaces_width_and_height() {
        let url = artwork_url("https://example.com/{w}x{h}bb.jpg", 300, 200);
        assert_eq!(url, "https://example.com/300x200bb.jpg");
    }

    #[test]
    fn artwork_url_replaces_the_crop_code() {
        let url = artwork_url("https://example.com/{w}x{h}{c}.{f}", 64, 64);
        assert_eq!(url, "https://example.com/64x64bb.jpg");
    }

    #[test]
    fn artwork_url_replaces_the_format() {
        let url = artwork_url("https://example.com/{w}x{h}bb.{f}", 64, 64);
        assert_eq!(url, "https://example.com/64x64bb.jpg");
    }

    #[test]
    fn artwork_url_replaces_every_occurrence() {
        assert_eq!(artwork_url("{w}/{w}", 1, 2), "1/1");
    }

    #[test]
    fn artwork_url_keeps_a_template_without_placeholders() {
        assert_eq!(
            artwork_url("https://example.com/a.jpg", 1, 1),
            "https://example.com/a.jpg"
        );
    }
}
