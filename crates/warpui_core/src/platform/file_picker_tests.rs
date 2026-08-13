use super::*;

#[test]
fn yaml_file_type_accepts_both_yaml_and_yml() {
    assert_eq!(FileType::Yaml.extensions(), &["yaml", "yml"]);
}

#[test]
fn markdown_file_type_accepts_md_and_markdown() {
    assert_eq!(FileType::Markdown.extensions(), &["md", "markdown"]);
}

#[test]
fn image_file_type_includes_heic_and_webp() {
    let extensions = FileType::Image.extensions();
    assert!(extensions.contains(&"heic"));
    assert!(extensions.contains(&"webp"));
}

#[test]
fn movie_file_type_covers_common_video_extensions() {
    let extensions = FileType::Movie.extensions();
    assert!(extensions.contains(&"mp4"));
    assert!(extensions.contains(&"mov"));
    assert_eq!(FileType::Movie.display_name(), "Movie");
}
