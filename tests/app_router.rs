//! Authoring declarations must preserve rmcp definitions and reject ambiguity.
#![cfg(all(feature = "macros", feature = "server"))]
#![forbid(unsafe_code)]

use apps_sdk::{
    AppRouter, MetadataError, RegistrationError, ResourceUi, StaticUiResource, UiResource,
    UiResourceRoute, UiResourceUri, UiUriError, app_router,
};
use mcp_apps_server as apps_sdk;
use rmcp::{model::MetaObject, tool};
use serde_json::json;

#[derive(StaticUiResource)]
#[ui_resource(
    id = "authoring/view",
    name = "view",
    html_file = "../examples/dashboard.html"
)]
struct View;
type ViewAlias = View;

fn tool_meta() -> MetaObject {
    MetaObject(
        json!({
            "vendor/setting": true,
            "ui": {"future": 7, "visibility": ["app"]},
            "ui/resourceUri": "ui://authoring/view"
        })
        .as_object()
        .unwrap()
        .clone(),
    )
}

struct Tools;
#[app_router(crate = apps_sdk)]
impl Tools {
    #[tool(name = "open_view", description = "Open the view", meta = tool_meta(),
        annotations(read_only_hint = true))]
    #[ui(resource = View)]
    fn open(&self) -> String {
        "View available".into()
    }

    #[rmcp::tool(description = "Refresh the same view")]
    #[ui(resource = ViewAlias)]
    async fn refresh(&self) -> String {
        "Refreshed".into()
    }

    #[tool(description = "Read without a UI")]
    #[cfg_attr(all(), allow(dead_code))]
    fn plain(&self) -> String {
        "Text".into()
    }

    // A missing resource type must not be referenced by disabled registration.
    #[tool(name = "open_view")]
    #[ui(resource = UnavailableView)]
    #[cfg(any())]
    fn disabled(&self) -> UnavailableOutput {
        unreachable!()
    }

    #[cfg_attr(all(), cfg(any()), allow(dead_code))]
    #[tool(name = "open_view")]
    #[ui(resource = AnotherUnavailableView)]
    fn conditionally_disabled(&self) -> String {
        unreachable!()
    }
}

#[test]
fn authoring_preserves_names_metadata_and_shared_resource_identity() {
    let router = Tools::app_router().unwrap();
    router.validate().unwrap();
    let resources = router.list_resources(None).unwrap().resources;
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "ui://authoring/view");
    let tools = router.list_tools(None).unwrap().tools;
    assert_eq!(tools.len(), 3);
    let mut expected = Tools::open_tool_attr();
    apps_sdk::ToolUi::new(UiResourceUri::new("ui://authoring/view").unwrap())
        .apply_to(&mut expected)
        .unwrap();
    assert_eq!(router.get_tool("open_view").unwrap(), expected);
    assert!(router.get_tool("open").is_none());
    assert_eq!(router.get_tool("plain").unwrap(), Tools::plain_tool_attr());
    let refresh = router.get_tool("refresh").unwrap();
    assert_eq!(
        refresh.meta.unwrap().0["ui"]["resourceUri"],
        "ui://authoring/view"
    );
}

struct DuplicateTools;
#[app_router]
impl DuplicateTools {
    #[tool(name = "same")]
    fn first(&self) -> String {
        String::new()
    }
    #[tool(name = "same")]
    fn second(&self) -> String {
        String::new()
    }
}

struct Generic<T: ?Sized>(T);
#[app_router]
impl<T: ?Sized + Send + Sync + 'static> Generic<T> {
    #[tool]
    #[ui(resource = View)]
    fn show(&self) -> String {
        String::new()
    }
}

// Existing metadata must not redirect an explicitly declared UI association.
struct BrokenMeta;
#[app_router]
impl BrokenMeta {
    #[tool(meta = MetaObject(json!({"ui": {"resourceUri": "ui://other/view"}})
        .as_object().unwrap().clone()))]
    #[ui(resource = View)]
    fn show(&self) -> String {
        String::new()
    }
}

#[test]
fn authoring_checks_duplicate_tool_names_and_existing_association_conflicts() {
    assert!(matches!(DuplicateTools::app_router(),
        Err(RegistrationError::DuplicateTool { name }) if name == "same"));
    assert!(matches!(
        BrokenMeta::app_router(),
        Err(RegistrationError::Metadata(MetadataError::Conflict { .. }))
    ));
    assert_eq!(
        Generic::<()>::app_router()
            .unwrap()
            .list_resources(None)
            .unwrap()
            .resources
            .len(),
        1
    );
}

struct Variant<const CHANGE: u8>;
impl<const CHANGE: u8> StaticUiResource for Variant<CHANGE> {
    fn resource() -> Result<UiResource, UiUriError> {
        Ok(UiResource::new(
            UiResourceUri::new("ui://authoring/view")?,
            if CHANGE == 1 { "another-name" } else { "view" },
        ))
    }
    fn html() -> &'static str {
        if CHANGE == 2 {
            "<html></html>"
        } else {
            View::html()
        }
    }
    fn ui() -> ResourceUi {
        let mut ui = ResourceUi::default();
        if CHANGE == 3 {
            ui.prefers_border = Some(true);
        }
        ui
    }
}

struct ConflictingViews;
#[app_router]
impl ConflictingViews {
    #[tool]
    #[ui(resource = View)]
    fn first(&self) -> String {
        String::new()
    }
    #[tool]
    #[ui(resource = Variant<2>)]
    fn second(&self) -> String {
        String::new()
    }
}

#[test]
fn static_reuse_requires_equal_descriptor_html_and_metadata() {
    let mut router = AppRouter::<()>::new();
    router.register_static::<View>().unwrap();
    let before = router.list_resources(None).unwrap();
    assert_eq!(
        router.ensure_static::<Variant<0>>().unwrap().as_str(),
        "ui://authoring/view"
    );
    for failure in [
        router.ensure_static::<Variant<1>>(),
        router.ensure_static::<Variant<2>>(),
        router.ensure_static::<Variant<3>>(),
    ] {
        assert!(
            matches!(failure, Err(RegistrationError::ConflictingResource { uri })
            if uri == "ui://authoring/view")
        );
    }
    assert_eq!(router.list_resources(None).unwrap(), before);
    assert!(matches!(
        router.register_static::<View>(),
        Err(RegistrationError::DuplicateResource { .. })
    ));
    assert!(matches!(
        ConflictingViews::app_router(),
        Err(RegistrationError::ConflictingResource { .. })
    ));

    let mut dynamic = AppRouter::<()>::new();
    dynamic
        .register_resource(UiResourceRoute::new(View::resource().unwrap(), |_, _| {
            Box::pin(async { Ok(View::html().to_owned()) })
        }))
        .unwrap();
    assert!(matches!(
        dynamic.ensure_static::<View>(),
        Err(RegistrationError::ConflictingResource { .. })
    ));
}
