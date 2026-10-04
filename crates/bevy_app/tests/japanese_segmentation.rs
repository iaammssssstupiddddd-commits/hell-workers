//! Dependency regression: exercise the exact Parley instance used by Bevy 0.19.
use parley::{
    FontContext, FontFamily, Layout, LayoutContext, PlainEditor, StyleProperty, WordBreak,
};

fn fonts() -> FontContext {
    let mut context = FontContext::new();
    let bytes = include_bytes!("../../../assets/fonts/NotoSansJP-VF.ttf");
    assert!(
        !context
            .collection
            .register_fonts(bytes.to_vec().into(), None)
            .is_empty()
    );
    context
}

#[test]
fn japanese_dictionary_word_boundaries_and_bounded_lines() {
    let mut fonts = fonts();
    let mut context = LayoutContext::<()>::new();
    // ICU's documented Japanese dictionary example: two words, 15 and 6 bytes.
    let text = "こんにちは世界";
    for mode in [WordBreak::Normal, WordBreak::BreakAll, WordBreak::KeepAll] {
        let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Noto Sans JP")));
        builder.push_default(StyleProperty::FontSize(16.0));
        builder.push_default(StyleProperty::WordBreak(mode));
        let mut layout: Layout<()> = builder.build(text);
        layout.break_all_lines(Some(200.0));
        assert!(layout.width().is_finite() && layout.width() > 0.0);
        let world = parley::Cluster::from_byte_index(&layout, 15).unwrap();
        assert!(
            world.is_word_boundary(),
            "dictionary boundary between greetings and world"
        );
    }
    let text = "橋の配置条件を確認します。建物の状態と保存結果を確認します。";
    let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named("Noto Sans JP")));
    builder.push_default(StyleProperty::FontSize(16.0));
    let mut layout: Layout<()> = builder.build(text);
    layout.break_all_lines(Some(160.0));
    assert!(layout.lines().count() > 1);
    assert!(layout.width() <= 161.0);
    assert!(
        layout
            .lines()
            .all(|line| text.is_char_boundary(line.text_range().start)
                && text.is_char_boundary(line.text_range().end))
    );
}

#[test]
fn japanese_editor_word_selection_and_replacement_preserve_utf8() {
    let mut fonts = fonts();
    let mut context = LayoutContext::<()>::new();
    let mut editor = PlainEditor::<()>::new(16.0);
    editor
        .edit_styles()
        .insert(StyleProperty::FontFamily(FontFamily::named("Noto Sans JP")));
    editor.set_text("こんにちは世界");
    editor.set_width(Some(200.0));
    {
        let mut driver = editor.driver(&mut fonts, &mut context);
        driver.move_to_text_end();
        driver.select_word_left();
    }
    // Parley treats legal line breaks as word boundaries too; Han selection
    // therefore remains one ideograph. Do not alter its editing policy here.
    assert_eq!(editor.selected_text(), Some("界"));
    editor
        .driver(&mut fonts, &mut context)
        .insert_or_replace_selection("建設");
    assert_eq!(editor.raw_text(), "こんにちは世建設");
    let mut driver = editor.driver(&mut fonts, &mut context);
    driver.move_to_text_end();
    driver.select_left();
    driver.delete_selection();
    assert_eq!(editor.raw_text(), "こんにちは世建");
}
