use super::*;
use image::Rgba;

#[test]
fn every_element_has_its_own_name() {
    let mut ids: Vec<&str> = RESULTS_ELEMENTS.iter().map(|e| e.id()).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total);
}

#[test]
fn names_round_trip_back_to_their_element() {
    for element in RESULTS_ELEMENTS {
        assert_eq!(ResultsElement::from_layer_type(element.id()), Some(element));
    }
    assert_eq!(ResultsElement::from_layer_type("text.static"), None);
    assert_eq!(ResultsElement::from_layer_type("results.nope"), None);
}

#[test]
fn bounds_ignore_transparent_padding() {
    let mut image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
    image.put_pixel(2, 3, Rgba([255, 0, 0, 255]));
    image.put_pixel(5, 6, Rgba([255, 0, 0, 128]));
    assert_eq!(opaque_bounds(&image), Some((2, 3, 4, 4)));
}

#[test]
fn an_empty_image_has_no_bounds() {
    assert!(opaque_bounds(&RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]))).is_none());
}

fn sprite(element: ResultsElement, w: u32, h: u32, tint: u8) -> ResultsElementSprite {
    ResultsElementSprite {
        element,
        x: 0,
        y: 0,
        image: RgbaImage::from_pixel(w, h, Rgba([tint, 0, 0, 255])),
    }
}

#[test]
fn the_atlas_wraps_onto_a_new_shelf_and_keeps_the_pixels() {
    let sprites = vec![
        sprite(ResultsElement::Panel, 200, 40, 10),
        sprite(ResultsElement::Score, 150, 20, 20),
        sprite(ResultsElement::Rank, 30, 10, 30),
    ];
    let atlas = pack_element_atlas(&sprites, 256).expect("atlas");
    assert!(atlas.image.width() <= 256);
    assert_eq!(atlas.placements.len(), 3);
    for sprite in &sprites {
        let placement = atlas
            .placements
            .iter()
            .find(|placement| placement.element == sprite.element)
            .expect("placement");
        assert_eq!(
            *atlas.image.get_pixel(placement.atlas_x, placement.atlas_y),
            *sprite.image.get_pixel(0, 0)
        );
    }
    // 60 and 50 do not share a shelf of 100, so the third row starts lower.
    assert!(atlas.image.height() > 40);
}

#[test]
fn the_background_stays_out_of_the_atlas() {
    let sprites = vec![
        sprite(ResultsElement::Background, 200, 100, 1),
        sprite(ResultsElement::Rank, 10, 10, 2),
    ];
    let atlas = pack_element_atlas(&sprites, 256).expect("atlas");
    assert_eq!(atlas.placements.len(), 1);
    assert_eq!(atlas.placements[0].element, ResultsElement::Rank);
}

#[test]
fn nothing_to_pack_means_no_atlas() {
    assert!(pack_element_atlas(&[], 256).is_none());
    let only_background = vec![sprite(ResultsElement::Background, 8, 8, 1)];
    assert!(pack_element_atlas(&only_background, 256).is_none());
}
