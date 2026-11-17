//! The dock's typeface: Inter (OFL-1.1), the face the ArtCraft apps use, in three weights.
//! egui's own fonts stay behind it as fallbacks for symbols and other scripts.

use egui::{FontData, FontDefinitions, FontFamily, FontId};

const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");

const SEMIBOLD_FAMILY: &str = "barcraft-semibold";
const BOLD_FAMILY: &str = "barcraft-bold";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    SemiBold,
    Bold,
}

/// A font of the given weight and size.
pub fn font(weight: Weight, size: f32) -> FontId {
    let family = match weight {
        Weight::Regular => FontFamily::Proportional,
        Weight::SemiBold => FontFamily::Name(SEMIBOLD_FAMILY.into()),
        Weight::Bold => FontFamily::Name(BOLD_FAMILY.into()),
    };
    FontId::new(size, family)
}

/// Makes Inter the interface font, with egui's fonts as fallbacks.
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallbacks = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, data) in [
        ("inter-regular", REGULAR),
        ("inter-semibold", SEMIBOLD),
        ("inter-bold", BOLD),
    ] {
        defs.font_data
            .insert(name.to_owned(), FontData::from_static(data).into());
    }
    let with_fallbacks = |first: &str| {
        let mut list = vec![first.to_owned()];
        list.extend(fallbacks.iter().cloned());
        list
    };
    defs.families
        .insert(FontFamily::Proportional, with_fallbacks("inter-regular"));
    defs.families.insert(
        FontFamily::Name(SEMIBOLD_FAMILY.into()),
        with_fallbacks("inter-semibold"),
    );
    defs.families.insert(
        FontFamily::Name(BOLD_FAMILY.into()),
        with_fallbacks("inter-bold"),
    );
    ctx.set_fonts(defs);
}
