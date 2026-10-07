//! A generic derive with an explicit crate alias matches handwritten declarations.
#![cfg(feature = "macros")]
#![forbid(unsafe_code)]
use apps_sdk::{
    MetaObject, MetadataError, ResourceUi, StaticUiResource, UiResource, UiResourceUri, UiUriError,
};
use mcp_apps_server as apps_sdk;
use std::marker::PhantomData;
mod metadata {
    pub fn meta() -> Result<super::MetaObject, super::MetadataError> {
        let mut meta = super::MetaObject::default();
        meta.0
            .insert("example/display".into(), serde_json::to_value("compact")?);
        Ok(meta)
    }
    pub fn ui() -> super::ResourceUi {
        let mut ui = super::ResourceUi::default();
        ui.prefers_border = Some(true);
        ui
    }
}
#[derive(apps_sdk::StaticUiResource)]
#[ui_resource(crate = apps_sdk, uri = "ui://derive/generic.html", name = "generic-dashboard", html = "../examples/dashboard.html", description = "Generic dashboard", ui = metadata::ui, meta = metadata::meta)]
struct Derived<T: ?Sized> {
    _marker: PhantomData<T>,
}
struct Manual;
impl StaticUiResource for Manual {
    fn resource() -> Result<UiResource, UiUriError> {
        Ok(UiResource::new(
            UiResourceUri::new("ui://derive/generic.html")?,
            "generic-dashboard",
        )
        .with_description("Generic dashboard"))
    }
    fn html() -> &'static str {
        include_str!("../examples/dashboard.html")
    }
    fn ui() -> ResourceUi {
        metadata::ui()
    }
    fn meta() -> Result<MetaObject, MetadataError> {
        metadata::meta()
    }
}
// No clone/schema/serialization traits: the derive must add no field bounds.
struct NoTraits;
#[derive(apps_sdk::StaticUiResource)]
#[ui_resource(crate = apps_sdk, id = "derive/generic.html", name = "generic-dashboard", html_file = "../examples/dashboard.html", description = "Generic dashboard", ui = metadata::ui, meta = metadata::meta)]
struct ShortDeclaration;
#[test]
fn generic_alias_derive_matches_manual_resource_and_metadata() {
    let derived = Derived::<NoTraits>::resource().unwrap();
    let manual = Manual::resource().unwrap();
    assert_eq!(
        serde_json::to_value(derived.descriptor()).unwrap(),
        serde_json::to_value(manual.descriptor()).unwrap()
    );
    let mut derived_meta = Derived::<NoTraits>::meta().unwrap();
    Derived::<NoTraits>::ui()
        .merge_into(&mut derived_meta)
        .unwrap();
    let mut manual_meta = Manual::meta().unwrap();
    Manual::ui().merge_into(&mut manual_meta).unwrap();
    let derived = derived
        .html_contents(Derived::<NoTraits>::html())
        .with_meta(derived_meta);
    let manual = manual.html_contents(Manual::html()).with_meta(manual_meta);
    assert_eq!(
        serde_json::to_value(derived).unwrap(),
        serde_json::to_value(manual).unwrap()
    );
    assert!(Derived::<str>::resource().is_ok());
    assert_eq!(
        serde_json::to_value(ShortDeclaration::resource().unwrap().descriptor()).unwrap(),
        serde_json::to_value(Manual::resource().unwrap().descriptor()).unwrap()
    );
    assert_eq!(ShortDeclaration::html(), Manual::html());
    assert_eq!(ShortDeclaration::meta().unwrap(), Manual::meta().unwrap());
    assert_eq!(
        ShortDeclaration::ui().to_meta().unwrap(),
        Manual::ui().to_meta().unwrap()
    );
}
