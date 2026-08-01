use super::*;
#[test]
fn only_vertical_canvases_get_a_band() {
    assert_eq!(vertical_hud_band(1920, 1080), None);
    assert_eq!(vertical_hud_band(1080, 1080), None);
    assert_eq!(vertical_hud_band(1080, 1920), Some(163));
}
#[test]
fn horizontal_config_is_untouched() {
    let user = HudConfig::default();
    let out = with_vertical_defaults(Some(user), 1920, 1080).unwrap();
    assert!(out.elements.score.is_none());
    assert!(out.elements.accuracy.is_none());
}
#[test]
fn the_score_fits_inside_the_band() {
    let band = vertical_hud_band(1080, 1920).unwrap() as f32;
    let score = with_vertical_defaults(None, 1080, 1920).unwrap().elements.score.unwrap();
    let bottom = score.y.unwrap() + score.height.unwrap();
    assert!(bottom <= band, "{bottom} sale de la franja de {band}");
}
/// A fixed height would shrink it to a third of its 16:9 size.
#[test]
fn the_accuracy_keeps_its_own_scale() {
    let accuracy = with_vertical_defaults(None, 1080, 1920).unwrap().elements.accuracy.unwrap();
    assert!(accuracy.y.is_some(), "se coloca");
    assert!(accuracy.height.is_none(), "pero no se redimensiona");
    assert!(accuracy.width.is_none());
}
#[test]
fn the_combo_and_the_judgement_stay_with_the_skin() {
    // skin.ini combo and score positions are part of its identity.
    let out = with_vertical_defaults(None, 1080, 1920).unwrap();
    assert!(out.elements.combo.is_none());
    assert!(out.elements.judgment_pop.is_none());
}
#[test]
fn what_the_user_authored_wins() {
    let mut user = HudConfig::default();
    user.elements.score = Some(HudElementConfig {
        y: Some(500.0),
        ..Default::default()
    });
    let out = with_vertical_defaults(Some(user), 1080, 1920).unwrap();
    let score = out.elements.score.unwrap();
    assert_eq!(score.y, Some(500.0));
}
